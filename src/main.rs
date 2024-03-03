extern crate tsunami;

use aws_sdk_ec2::types::InstanceType;
use tsunami::{Machine, MachineSetup, TsunamiBuilder};
use std::collections::HashMap;

#[tokio::main]
async fn main() {

    // export AWS_ACCESS_KEY_ID=
    // export AWS_SECRET_ACCESS_KEY= 

    let mut b = TsunamiBuilder::default();
    b.add_set(
        "server",
        1,
        MachineSetup::new(InstanceType::T2Medium, "ami-0440d3b780d96b29d", |ssh| { 
            ssh.cmd("cat /etc/hostname").map(|out| {
                println!("{}", out);
            })
        }, "key1"),
    );
    b.add_set(
        "client",
        3,
        MachineSetup::new(InstanceType::T2Micro, "ami-0440d3b780d96b29d", |ssh| {
            ssh.cmd("date").map(|out| {
                println!("{}", out);
            })
        }, "key1"),
    );

    let r = b.run(|vms: HashMap<String, Vec<Machine>>| {
        println!("==> {}", vms["server"][0].private_ip);
        for c in &vms["client"] {
            println!(" -> {}", c.private_ip);
        }
        Ok(())
    }).await;   
    println!("{:#?}", r);
}
