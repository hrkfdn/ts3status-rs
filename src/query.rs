use std::{
    sync::{Arc, RwLock},
    time::Instant,
};

use log::{error, info, trace};
use serde::Serialize;
use ts3_query::*;

use crate::Config;

// Update server status every 20 seconds at the earliest
pub const CACHE_LIFETIME: u64 = 20;

#[derive(Clone, Default, Serialize)]
pub struct Client {
    pub nickname: String,
    pub country: String,
    pub input_muted: bool,
    pub output_muted: bool,
    pub away: bool,
}

impl From<&OnlineClientFull> for Client {
    fn from(client: &OnlineClientFull) -> Self {
        Self {
            nickname: client.client_nickname.clone(),
            country: client.client_country.clone(),
            input_muted: client.client_input_muted,
            output_muted: client.client_output_muted,
            away: client.client_away,
        }
    }
}

#[derive(Clone, Default, Serialize)]
pub struct ChannelNode {
    pub id: u64,
    pub name: String,
    pub clients: Vec<Client>,
    pub children: Vec<ChannelNode>,
}

pub struct StatusCache {
    pub last_update: Instant,
    pub root: ChannelNode,
}

impl ChannelNode {
    pub fn add_to_parent(&mut self, parent_id: u64, channel: &ChannelNode) {
        if self.id == parent_id {
            self.children.push(channel.clone());
        } else {
            for child in &mut self.children {
                child.add_to_parent(parent_id, channel);
            }
        }
    }
}

fn channel_tree(channels: Vec<ChannelFull>, clients: Vec<OnlineClientFull>) -> ChannelNode {
    let mut root = ChannelNode {
        id: 0,
        name: "Root".to_string(),
        clients: Vec::new(),
        children: Vec::new(),
    };

    for channel in channels {
        let node = ChannelNode {
            id: channel.cid,
            name: channel.channel_name,
            clients: clients
                .iter()
                .filter(|c| c.client_type == 0 && c.cid == channel.cid)
                .map(|c| c.into())
                .collect(),
            children: Vec::new(),
        };
        root.add_to_parent(channel.pid, &node);
    }

    root
}

pub async fn fetch_status(
    cfg: &Config,
    cache: &Arc<RwLock<StatusCache>>,
) -> Result<ChannelNode, Ts3Error> {
    info!("Fetching TS3 server status");

    let last_update = cache.read().expect("can't readlock cache").last_update;
    let root = if last_update.elapsed().as_secs() > CACHE_LIFETIME {
        info!(
            "Status is {} seconds old, updating cache",
            last_update.elapsed().as_secs()
        );
        let mut client = QueryClient::new((&*cfg.ts3_host, cfg.ts3_port))?;

        client.login(&cfg.user, &cfg.password)?;
        client.select_server_by_id(cfg.ts3_server_id)?;

        let channels = client.channels_full()?;
        trace!("channels: {:?}", channels);

        let clients = client.online_clients_full()?;
        trace!("clients: {:?}", clients);
        client.logout()?;

        let root = channel_tree(channels, clients);
        if let Ok(mut cache) = cache.write() {
            cache.last_update = Instant::now();
            cache.root = root.clone();
        } else {
            error!("Can not write lock cache");
        }
        root
    } else {
        info!("Using cached server status");
        cache.read().expect("can't readlock cache").root.clone()
    };

    Ok(root)
}
