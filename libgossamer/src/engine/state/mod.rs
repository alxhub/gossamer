use std::collections::{HashMap, HashSet};

pub type ChannelId = u32;
pub type ClientId = u32;
pub type ServerId = u32;
pub type SubnetId = u32;

pub struct Channel {
  pub id: ChannelId,
  pub subnet: SubnetId,
  pub name: String,
  pub topic: String,
  pub members: HashMap<ClientId, Membership>,
}

pub struct Membership {
  pub has_op: bool,
}

pub struct Client {
  pub id: ClientId,
  pub server: ServerId,
  pub subnet: SubnetId,
  pub nick: String,
  pub lnick: String,
  pub ident: String,
  pub host: String,
  pub gecos: String,

  pub channels: HashSet<ChannelId>,
}

pub struct Server {
  pub id: ServerId,
  pub name: String,
  pub lname: String,
  pub link: Option<Link>,
  pub clients: HashSet<ClientId>,
  pub downlinks: HashSet<ServerId>,
}

#[derive(Copy, Clone)]
pub struct Link {
  pub hub: ServerId,
  pub route: ServerId,
}

pub struct Subnet {
  pub id: SubnetId,
  pub name: String,
  pub lname: String,
  pub clients: HashSet<ClientId>,
  pub clients_by_lnick: HashMap<String, ClientId>,
  pub channels_by_lname: HashMap<String, ChannelId>,
}

/// State of an IRC network, completely independent of any external dependencies.
pub struct State {
  next_client_id: ClientId,
  next_server_id: ServerId,
  next_channel_id: ChannelId,
  next_subnet_id: SubnetId,

  clients: HashMap<ClientId, Client>,
  servers: HashMap<ServerId, Server>,
  servers_by_lname: HashMap<String, ServerId>,
  channels: HashMap<ChannelId, Channel>,
  subnets: HashMap<SubnetId, Subnet>,
  subnets_by_lname: HashMap<String, SubnetId>,
}

impl State {
  pub fn new(server_name: String) -> State {
    // Construct the initial, empty network state.
    let mut state = State {
      // ServerId 0 is reserved for the local server.
      next_server_id: 1,

      // The rest of the ids start at 0.
      next_channel_id: 0,
      next_client_id: 0,
      next_subnet_id: 0,

      channels: HashMap::new(),
      clients: HashMap::new(),
      servers: HashMap::new(),
      servers_by_lname: HashMap::new(),
      subnets: HashMap::new(),
      subnets_by_lname: HashMap::new(),
    };

    // Construct and add the first server (the local one).
    let server = Server {
      id: 0,
      lname: server_name.to_lowercase(),
      name: server_name,
      link: None,
      clients: HashSet::new(),
      downlinks: HashSet::new(),
    };
    state
      .servers_by_lname
      .insert(server.lname.clone(), server.id);
    state.servers.insert(server.id, server);

    state
  }
}

impl State {
  pub fn client_by_id(&self, id: ClientId) -> &Client {
    self.clients.get(&id).expect("Invalid ClientId")
  }

  pub fn client_by_nick(&self, subnet: SubnetId, lnick: &str) -> Option<ClientId> {
    self
      .subnet_by_id(subnet)
      .clients_by_lnick
      .get(lnick)
      .map(|id| *id)
  }

  pub fn client_add(
    &mut self,
    nick: String,
    ident: String,
    host: String,
    gecos: String,
    server: ServerId,
    subnet: SubnetId,
  ) -> Result<ClientId, Error> {
    // Allocate an id.
    let id = self.next_client_id;

    // Allocate the client itself.
    let client = Client {
      id,
      server,
      subnet,

      lnick: nick.clone().to_lowercase(),
      nick: nick,
      ident,
      host,
      gecos,

      channels: HashSet::new(),
    };

    let subnet = self.subnets.get_mut(&subnet).unwrap();
    let server = self.servers.get_mut(&server).unwrap();

    // Check whether this nickname is taken.
    if let Some(existing) = subnet.clients_by_lnick.get(&client.lnick) {
      return Err(Error::NickAlreadyInUse(*existing));
    }

    subnet.clients.insert(id);
    subnet.clients_by_lnick.insert(client.lnick.clone(), id);
    server.clients.insert(id);
    self.clients.insert(id, client);

    self.next_client_id += 1;
    Ok(id)
  }

  pub fn client_iter<'a>(&'a self) -> impl Iterator<Item = &'a ClientId> + 'a {
    self.clients.keys()
  }

  pub fn client_remove(&mut self, id: ClientId) {
    let client = self.clients.remove(&id).unwrap();
    let subnet = self.subnets.get_mut(&client.subnet).unwrap();
    subnet.clients.remove(&id);
    subnet.clients_by_lnick.remove(&client.lnick);

    let server = self.servers.get_mut(&client.server).unwrap();
    server.clients.remove(&id);

    for channel in client.channels {
      self.channel_remove_member(channel, id);
    }
  }
}

impl State {
  pub fn channel_by_id(&self, id: ChannelId) -> &Channel {
    self.channels.get(&id).expect("Invalid ChannelId")
  }

  pub fn channel_remove_member(&mut self, id: ChannelId, member_id: ClientId) {}
}

impl State {
  pub fn subnet_by_id(&self, id: SubnetId) -> &Subnet {
    self.subnets.get(&id).expect("Invalid SubnetId")
  }

  pub fn subnet_by_name(&self, lname: &str) -> Option<SubnetId> {
    self.subnets_by_lname.get(lname).map(|id| *id)
  }

  pub fn subnet_add(&mut self, name: String) -> Result<SubnetId, Error> {
    if let Some(existing) = self.subnets_by_lname.get(&name) {
      return Err(Error::SubnetNameAlreadyInUse(*existing));
    }

    let id = self.next_subnet_id;
    self.next_subnet_id += 1;

    let subnet = Subnet {
      id,
      lname: name.clone().to_lowercase(),
      name: name,
      clients: HashSet::new(),
      clients_by_lnick: HashMap::new(),
      channels_by_lname: HashMap::new(),
    };

    self.subnets_by_lname.insert(subnet.lname.clone(), id);
    self.subnets.insert(id, subnet);

    Ok(id)
  }

  pub fn subnet_iter<'a>(&'a self) -> impl Iterator<Item = &'a SubnetId> + 'a {
    self.subnets.keys()
  }
}

impl State {
  pub fn server_by_id(&self, id: ServerId) -> &Server {
    self.servers.get(&id).expect("Invalid ServerId")
  }

  pub fn server_by_name(&self, lname: &str) -> Option<ServerId> {
    self.servers_by_lname.get(lname).map(|id| *id)
  }

  pub fn server_add(&mut self, name: String, hub: ServerId) -> Result<ServerId, Error> {
    let lname = name.to_lowercase();

    if let Some(existing) = self.server_by_name(&lname) {
      return Err(Error::ServerNameAlreadyExists(existing));
    }

    let id = self.next_server_id;
    self.next_server_id += 1;

    let link = if let Some(link) = self.server_by_id(hub).link {
      Link {
        hub,
        route: link.route,
      }
    } else {
      Link { hub: 0, route: id }
    };

    self
      .servers
      .get_mut(&link.hub)
      .unwrap()
      .downlinks
      .insert(id);

    let link = Some(link);

    let server = Server {
      id,
      name,
      lname,
      link,
      clients: HashSet::new(),
      downlinks: HashSet::new(),
    };

    self.servers_by_lname.insert(server.lname.clone(), id);
    self.servers.insert(id, server);

    Ok(id)
  }

  pub fn server_count(&self) -> usize {
    self.servers.len()
  }

  pub fn server_remove(&mut self, id: ServerId) {
    let server = self.servers.remove(&id).unwrap();
    if server.clients.len() > 0 {
      panic!("cannot remove a non-empty server");
    }
    for link in &server.downlinks {
      self.server_remove(*link);
    }

    if let Some(link) = server.link {
      match self.servers.get_mut(&link.hub) {
        Some(hub) => {
          hub.downlinks.remove(&id);
        }
        None => (),
      };
    }
  }

  pub fn calculate_netsplit_affected_clients(&self, id: ServerId) -> HashSet<ClientId> {
    let mut set = HashSet::new();
    self.calculate_netsplit_affected_clients_helper(id, &mut set);
    set
  }

  fn calculate_netsplit_affected_clients_helper(&self, id: ServerId, set: &mut HashSet<ClientId>) {
    let server = self.servers.get(&id).unwrap();
    for client in &server.clients {
      set.insert(*client);
    }
    for other in server.downlinks.iter() {
      self.calculate_netsplit_affected_clients_helper(*other, set);
    }
  }
}

#[derive(Debug)]
pub enum Error {
  NickAlreadyInUse(ClientId),
  SubnetNameAlreadyInUse(SubnetId),
  ServerNameAlreadyExists(ServerId),
}
