use futures::{SinkExt, StreamExt};
use ldap3_server::{simple::*, LdapCodec};
use std::{convert::TryFrom, net, str::FromStr};
use tokio::net::{TcpListener, TcpStream};
use tokio_util::codec::{FramedRead, FramedWrite};

async fn handle_client(socket: TcpStream, addr: net::SocketAddr) {
    let (r, w) = tokio::io::split(socket);
    let mut reqs = FramedRead::new(r, LdapCodec);
    let mut resp = FramedWrite::new(w, LdapCodec);

    while let Some(msg) = reqs.next().await {
        let server_op = match msg.map_err(|_e| ()).and_then(ServerOps::try_from) {
            Ok(v) => v,
            Err(_) => {
                let _err = resp
                    .send(DisconnectionNotice::gen(
                        LdapResultCode::Other,
                        "Internal Server Error",
                    ))
                    .await;
                let _err = resp.flush().await;
                return;
            }
        };

        let result = match server_op {
            ServerOps::SimpleBind(sbr) => {
                println!("{}  Bind: {} {}", addr, sbr.dn, sbr.pw);
                vec![sbr.gen_success()]
            }
            ServerOps::Search(sr) => {
                println!("{}  Search: {}", addr, sr.base);
                vec![sr.gen_success()]
            }
            ServerOps::Unbind(_) => {
                println!("{}  UnBind", addr);
                return;
            }
            ServerOps::Whoami(wr) => {
                println!("{}  Whoami", addr);
                vec![wr.gen_success("dn: Anonymous")]
            }
        };

        for rmsg in result.into_iter() {
            if resp.send(rmsg).await.is_err() {
                return;
            }
        }

        if resp.flush().await.is_err() {
            return;
        }
    }
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let bind_addr = if args.len() < 2 {
        "0.0.0.0:389"
    } else {
        args[1].as_str()
    };

    let addr = net::SocketAddr::from_str(bind_addr).unwrap();
    let listener = Box::new(TcpListener::bind(&addr).await.unwrap());

    tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((socket, paddr)) => {
                    tokio::spawn(handle_client(socket, paddr));
                }
                Err(_e) => {}
            }
        }
    });

    println!("started ldap://{} ...", bind_addr);
    tokio::signal::ctrl_c().await.unwrap();
}
