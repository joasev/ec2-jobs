extern crate failure;
extern crate ssh2;

use aws_config::{meta::region::RegionProviderChain, BehaviorVersion};
use aws_sdk_ec2::{Client, types::{InstanceType, Tag, ResourceType, TagSpecification, Filter, InstanceStateName, Instance}};
use failure::Error;
use std::collections::HashMap;
use tokio::time::Duration;


mod ssh;

pub struct Machine {
    pub ssh: Option<ssh::Session>,
    pub instance_type: InstanceType,
    pub private_ip: String,
    pub public_dns: String,
    pub instance_id: String,
    pub key_name: String,
}


pub struct MachineSetup {
    instance_type: InstanceType,
    ami: String,
    setup: Box<dyn Fn(&mut ssh::Session) -> Result<(), Error>>,
    key_name: String,
}

impl MachineSetup {
    pub fn new<F>(instance_type: InstanceType, ami: &str, setup: F, key: &str) -> Self
    where
        F: Fn(&mut ssh::Session) -> Result<(), Error> + 'static,
    {
        MachineSetup {
            instance_type: instance_type,
            ami: ami.to_string(),
            setup: Box::new(setup),
            key_name: key.to_string(),
        }
    }
}

pub struct TsunamiBuilder {
    descriptors: HashMap<String, (MachineSetup, u16)>,
    max_duration: i64,
}

impl Default for TsunamiBuilder {
    fn default() -> Self {
        TsunamiBuilder {
            descriptors: Default::default(),
            max_duration: 60,
        }
    }
}

impl TsunamiBuilder {
    pub fn add_set(&mut self, name: &str, number: u16, setup: MachineSetup) {
        // TODO: what if name is already in use?
        self.descriptors.insert(name.to_string(), (setup, number));
    }

    pub fn set_max_duration(&mut self, hours: u8) {
        self.max_duration = hours as i64 * 60;
    }

    pub async fn run<F>(self, f: F) -> Result<(), Error>
    where
        F: FnOnce(HashMap<String, Vec<Machine>>) -> Result<(), Error>,
    {
        
        let region_provider = RegionProviderChain::default_provider().or_else("us-east-1");
        let config = aws_config::defaults(BehaviorVersion::latest())
            .region(region_provider)
            .load()
            .await;
        let client = Client::new(&config);
        
        let mut setup_fns = HashMap::new();
        // 1. Launch instances
        let mut instance_ids = Vec::new();
        let mut id_to_name = HashMap::new();
        for (name, (setup, number)) in self.descriptors {
            setup_fns.insert(name.clone(), setup.setup);

            let name_tag = Tag::builder()
                .key("Name")
                .value(name.clone()) 
                .build();
            let tag_specification = TagSpecification::builder()
                .resource_type(ResourceType::Instance)
                .tags(name_tag)
                .build();
            let run_instances = client.run_instances()
                .image_id(setup.ami) 
                .instance_type(setup.instance_type) 
                .min_count(i32::from(number))
                .max_count(i32::from(number))
                .tag_specifications(tag_specification)
                .key_name(&setup.key_name) 
                .send()
                .await?;

            if let Some(instances) = run_instances.instances {
                instance_ids.extend(instances.iter().filter_map(|i| i.instance_id.clone()));
                instances.into_iter().for_each(|inst| {
                    if let Some(instance_id) = inst.instance_id {
                        id_to_name.insert(instance_id, name.clone());
                    }
                });
                
            } 
        }
        
        // 2. Wait until all instances are up
        let ready_machines = Self::instances_all_ready(&client, instance_ids.clone()).await?;
        println!("Finished waiting");
        ready_machines.iter().for_each(|m| println!("Public dns: {}", m.public_dns));
        
        
        let mut machines: HashMap<String, Vec<Machine>> = HashMap::new();
        
        for machine in ready_machines {
            let name = id_to_name[&machine.instance_id].clone();
            machines.entry(name).or_insert_with(Vec::new).push(machine);
        }
        
        
        // 3. Once an instance is ready, run setup closure
        for (name, machines) in &mut machines {
            let f = &setup_fns[name];
            for machine in machines {

                println!("Attempting to ssh to: {:#?} with key of name: {}",machine.public_dns, format!("{}.pem", machine.key_name));
                
                let mut sess = ssh::Session::connect(&format!("{}:22", machine.public_dns), &format!("{}.pem", machine.key_name))
                    .map_err(Error::from)
                    .map_err(|e| {
                        e.context(format!(
                            "failed to ssh to {} machine {}",
                            name, machine.public_dns
                        ))
                    })?;

                f(&mut sess).map_err(|e| {
                    e.context(format!("setup procedure for {} machine failed", name))
                })?;

                machine.ssh = Some(sess);
            }
        }

        // 4. invoke F with Machine descriptors
        f(machines).map_err(|e| e.context("tsunami main routine failed"))?;
    
    

        // 5. Terminate all instances
        if let Err(e) = Self::terminate_instances(&client, instance_ids).await {
            eprintln!("Failed to terminate instances: {}", e);
        } else {
            println!("Instances terminated successfully.");
        }

        Ok(())
    }

    async fn instances_all_ready(client: &Client, instance_ids: Vec<String>) -> Result<Vec<Machine>, Error> {
        let mut machines = Vec::new();
        for id in instance_ids {
            let mut is_ready = false;
            while !is_ready {
                if let Some(machine) =  Self::is_instance_ready(&client, &id).await? {
                    is_ready = true;
                    machines.push(machine);
                } else {
                    println!("Is not ready");
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
                
            }
            println!("Is ready");

        }
        Ok(machines)
    }
    async fn is_instance_ready(client: &Client, instance_id: &str) -> Result<Option<Machine>, Error> {
        let filters = vec![
            Filter::builder()
                .name("instance-id")
                .values(instance_id)
                .build(),
        ];

        let resp = client.describe_instances()
            .set_filters(Some(filters))
            .send()
            .await?;
        
        for reservation in resp.reservations.unwrap_or_default() {
            for instance in reservation.instances.unwrap_or_default() {

                match instance {
                    Instance {
                        instance_id: Some(instance_id),
                        instance_type: Some(instance_type),
                        private_ip_address: Some(private_ip),
                        public_dns_name: Some(public_dns),
                        state: Some(state),
                        key_name: Some(key_name),
                        ..
                    } => {
                        if state.name == Some(InstanceStateName::Running) {
                            let machine = Machine {
                                ssh: None,
                                instance_type,
                                private_ip,
                                public_dns,
                                instance_id, 
                                key_name,
                            };

                            return Ok(Some(machine))
                        }
                    }
                    _ => {
                        return Ok(None)
                    }
                }
            }
        }
        Ok(None)
    }

    async fn terminate_instances(client: &Client, instance_ids: Vec<String>) -> Result<(), Error> {
        let request = client.terminate_instances()
            .set_instance_ids(Some(instance_ids));
    
        let response = request.send().await?;
    
        // Print the IDs of terminated instances
        if let Some(terminating_instances) = response.terminating_instances {
            for instance in terminating_instances {
                if let Some(instance_id) = instance.instance_id {
                    println!("Terminated instance ID: {}", instance_id);
                }
            }
        }
    
        Ok(())
    }

    
}
