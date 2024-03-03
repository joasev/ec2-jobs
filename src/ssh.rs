use ssh2;
use std::{net::{self, TcpStream}, path::Path, time::Duration};
use failure::Error;

pub struct Session {
    ssh: ssh2::Session,
}

impl Session {
    pub(crate) fn connect<A: net::ToSocketAddrs>(addr: A, key: &str) -> Result<Self, Error> {
        let mut i = 0;

        println!("TcpStream connect");
        let tcp = loop {
            match TcpStream::connect(&addr) {
                Ok(s) => break s,
                Err(_) if i <= 1 => { 
                    println!("Attempt {} failed, retrying in 10 seconds...", i + 1);
                    i += 1;
                    std::thread::sleep(Duration::from_secs(10)); 
                },
                Err(e) => Err(Error::from(e).context("failed to connect to ssh port"))?,
            }
        };

        println!("Session");
        let mut sess = ssh2::Session::new()?;

        println!("Handshake");
        /* 
        sess.handshake(&tcp)
            .map_err(Error::from)
            .map_err(|e| e.context("failed to perform ssh handshake"))?; */
        sess.set_tcp_stream(tcp);
        sess.handshake()?;

        // TODO
        /* sess.userauth_agent("ec2-user")
            .map_err(Error::from)
            .map_err(|e| e.context("failed to authenticate ssh session"))?; */
        println!("{}", key);
        sess.userauth_pubkey_file("ec2-user", None, Path::new(key), None)?;


        Ok(Session {
            ssh: sess,
        })
    }

    pub fn cmd(&mut self, cmd: &str) -> Result<String, Error> {
        use std::io::Read;

        let mut channel = self.ssh
            .channel_session()
            .map_err(Error::from)
            .map_err(|e| {
                e.context(format!(
                    "failed to create ssh channel for command '{}'",
                    cmd
                ))
            })?;

        channel
            .exec(cmd)
            .map_err(Error::from)
            .map_err(|e| e.context(format!("failed to execute command '{}'", cmd)))?;

        let mut s = String::new();
        channel
            .read_to_string(&mut s)
            .map_err(Error::from)
            .map_err(|e| e.context(format!("failed to read results of command '{}'", cmd)))?;

        channel
            .wait_close()
            .map_err(Error::from)
            .map_err(|e| e.context(format!("command '{}' never completed", cmd)))?;

        // TODO: check channel.exit_status()
        Ok(s)
    }
}

use std::ops::{Deref, DerefMut};
impl Deref for Session {
    type Target = ssh2::Session;
    fn deref(&self) -> &Self::Target {
        &self.ssh
    }
}

impl DerefMut for Session {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.ssh
    }
}
