use crate::{
    types::MessageFrame,
    utils::{create_downstream, create_upstream, message_from_frame, wait_for_client},
};
use async_channel::Sender;
use std::net::SocketAddr;
use tokio::net::TcpStream;
use tracing::info;

pub struct MockDownstream {
    upstream_address: SocketAddr,
}

impl MockDownstream {
    pub fn new(upstream_address: SocketAddr) -> Self {
        Self { upstream_address }
    }

    pub async fn start(&self) -> Sender<MessageFrame> {
        let upstream_address = self.upstream_address;
        let (upstream_receiver, upstream_sender) = create_upstream(loop {
            match TcpStream::connect(upstream_address).await {
                Ok(stream) => break stream,
                Err(_) => {
                    println!("MockDownstream: unable to connect to upstream, retrying");
                }
            }
        })
        .await
        .expect("Failed to create upstream");
        tokio::spawn(async move {
            while let Ok(mut frame) = upstream_receiver.recv().await {
                let (msg_type, msg) = message_from_frame(&mut frame);
                info!(
                    "MockDownstream: received message from upstream: {} {}",
                    msg_type, msg
                );
            }
        });
        upstream_sender
    }
}

pub struct MockUpstream {
    listening_address: SocketAddr,
}

impl MockUpstream {
    pub fn new(listening_address: SocketAddr) -> Self {
        Self { listening_address }
    }

    pub async fn start(&self) -> Sender<MessageFrame> {
        let listening_address = self.listening_address;

        // Create proxy channel - return this immediately
        let (proxy_sender, proxy_receiver) = async_channel::unbounded::<MessageFrame>();

        tokio::spawn(async move {
            // Wait for client connection in background
            let (downstream_receiver, downstream_sender) =
                create_downstream(wait_for_client(listening_address).await)
                    .await
                    .expect("Failed to connect to downstream");

            // Spawn task to receive from downstream
            tokio::spawn(async move {
                while let Ok(mut frame) = downstream_receiver.recv().await {
                    let (msg_type, msg) = message_from_frame(&mut frame);
                    info!(
                        "MockUpstream: received message from downstream: {} {}",
                        msg_type, msg
                    );
                }
            });

            // Forward messages from proxy to actual downstream
            while let Ok(frame) = proxy_receiver.recv().await {
                if downstream_sender.send(frame).await.is_err() {
                    break;
                }
            }
        });

        proxy_sender
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        interceptor::MessageDirection, start_sniffer, start_template_provider,
        template_provider::DifficultyLevel,
    };
    use std::{convert::TryInto, net::TcpListener};
    use stratum_apps::{
        stratum_core::{
            codec_sv2::StandardEitherFrame,
            common_messages_sv2::{Protocol, SetupConnection, SetupConnectionSuccess, *},
            parsers_sv2::{AnyMessage, CommonMessages},
        },
        utils::types::Sv2Frame,
    };

    #[tokio::test]
    async fn test_mock_downstream() {
        let (_tp, socket) = start_template_provider(None, DifficultyLevel::Low);
        let (sniffer, sniffer_addr) = start_sniffer("", socket, false, vec![], None);
        let mock_downstream = MockDownstream::new(sniffer_addr);
        let send_to_upstream = mock_downstream.start().await;
        let setup_connection =
            AnyMessage::Common(CommonMessages::SetupConnection(SetupConnection {
                protocol: Protocol::TemplateDistributionProtocol,
                min_version: 2,
                max_version: 2,
                flags: 0,
                endpoint_host: b"0.0.0.0".to_vec().try_into().unwrap(),
                endpoint_port: 8081,
                vendor: b"Bitmain".to_vec().try_into().unwrap(),
                hardware_version: b"901".to_vec().try_into().unwrap(),
                firmware: b"abcX".to_vec().try_into().unwrap(),
                device_id: b"89567".to_vec().try_into().unwrap(),
            }));
        let message = StandardEitherFrame::<AnyMessage<'_>>::Sv2(
            Sv2Frame::from_message(setup_connection, MESSAGE_TYPE_SETUP_CONNECTION, 0, false)
                .expect("Failed to create the frame"),
        );
        send_to_upstream.send(message).await.unwrap();
        sniffer
            .wait_for_message_type(
                MessageDirection::ToDownstream,
                MESSAGE_TYPE_SETUP_CONNECTION_SUCCESS,
            )
            .await;
    }

    #[tokio::test]
    async fn test_mock_upstream() {
        let port = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let upstream_socket_addr = SocketAddr::from(([127, 0, 0, 1], port));
        let (sniffer, sniffer_addr) = start_sniffer("", upstream_socket_addr, false, vec![], None);
        let mock_downstream = MockDownstream::new(sniffer_addr);
        let mock_upstream = MockUpstream::new(upstream_socket_addr);
        let send_to_downstream = mock_upstream.start().await;
        let send_to_upstream = mock_downstream.start().await;
        let setup_connection =
            AnyMessage::Common(CommonMessages::SetupConnection(SetupConnection {
                protocol: Protocol::TemplateDistributionProtocol,
                min_version: 2,
                max_version: 2,
                flags: 0,
                endpoint_host: b"0.0.0.0".to_vec().try_into().unwrap(),
                endpoint_port: 8081,
                vendor: b"Bitmain".to_vec().try_into().unwrap(),
                hardware_version: b"901".to_vec().try_into().unwrap(),
                firmware: b"abcX".to_vec().try_into().unwrap(),
                device_id: b"89567".to_vec().try_into().unwrap(),
            }));
        let message = StandardEitherFrame::<AnyMessage<'_>>::Sv2(
            Sv2Frame::from_message(setup_connection, MESSAGE_TYPE_SETUP_CONNECTION, 0, false)
                .expect("Failed to create the frame"),
        );
        send_to_upstream.send(message).await.unwrap();

        let success_message = StandardEitherFrame::<AnyMessage<'_>>::Sv2(
            Sv2Frame::from_message(
                AnyMessage::Common(CommonMessages::SetupConnectionSuccess(
                    SetupConnectionSuccess {
                        used_version: 2,
                        flags: 0,
                    },
                )),
                MESSAGE_TYPE_SETUP_CONNECTION_SUCCESS,
                0,
                false,
            )
            .expect("Failed to create the frame"),
        );
        send_to_downstream.send(success_message).await.unwrap();
        sniffer
            .wait_for_message_type(
                MessageDirection::ToDownstream,
                MESSAGE_TYPE_SETUP_CONNECTION_SUCCESS,
            )
            .await;
    }
}
