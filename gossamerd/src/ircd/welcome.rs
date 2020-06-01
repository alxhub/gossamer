use crate::client::ClientEvent;
use futures::channel::mpsc;
use futures::sink::SinkExt;
use libgossamer::engine::state::ClientId;
use libgossamer::engine::Network;

use super::Ircd;

impl Ircd {
  pub async fn send_welcome(
    &mut self,
    network: &mut Network,
    id: ClientId,
    mut client_tx: mpsc::Sender<ClientEvent>,
  ) -> Result<(), mpsc::SendError> {
    let client = network.state.client_by_id(id);
    let nick = &client.nick;

    let server_name = &network.state.server_by_id(0).name;

    client_tx
      .send(ClientEvent::Send(format!(
        ":{} 001 {} :Welcome to the NAME Internet Relay Chat Network {}\r\n",
        server_name, nick, nick
      )))
      .await?;
    client_tx
      .send(ClientEvent::Send(format!(
        ":{} 002 {} :Your host is {}, running version VERSION\r\n",
        server_name, nick, server_name
      )))
      .await?;
    client_tx
      .send(ClientEvent::Send(format!(
        ":{} 003 {} :This server was created DATE\r\n",
        server_name, nick
      )))
      .await?;

    // TODO: add correct mode list.
    client_tx
      .send(ClientEvent::Send(format!(
        ":{} 004 {} {} gossamircd-0.1 CDGNRSUWagilopqrswxyz BCIMNORSabcehiklmnopqstvz Iabehkloqv\r\n",
        server_name, nick, server_name
      )))
      .await?;

    // TODO: add correct feature list.
    client_tx
        .send(ClientEvent::Send(format!(
          ":{} 005 {} CALLERID CASEMAPPING=rfc1459 DEAF=D KICKLEN=180 MODES=4 PREFIX=(qaohv)~&@%+ STATUSMSG=~&@%+ EXCEPTS=e INVEX=I NICKLEN=30 NETWORK=Rizon MAXLIST=beI:250 MAXTARGETS=4 :are supported by this server\r\n",
          server_name, nick
        )))
        .await?;
    client_tx
      .send(ClientEvent::Send(format!(
        ":{} 005 {} CHANTYPES=# CHANLIMIT=#:250 CHANNELLEN=50 TOPICLEN=390 CHANMODES=beI,k,l,BCMNORScimnpstz WATCH=60 NAMESX UHNAMES AWAYLEN=180 ELIST=CMNTU SAFELIST KNOCK :are supported by this server\r\n",
        server_name, nick
      )))
      .await?;

    client_tx
      .send(ClientEvent::Send(format!(
        ":{} MODE {} :+ix\r\n",
        server_name, nick
      )))
      .await?;

    Ok(())
  }
}
