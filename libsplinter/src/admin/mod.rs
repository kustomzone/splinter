// Copyright 2019 Cargill Incorporated
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::collections::HashMap;
use std::fmt::Write;
use std::sync::{Arc, Mutex};

use openssl::hash::{hash, MessageDigest};
use protobuf::{self, Message};

use crate::actix_web::{web, Error as ActixError, HttpRequest, HttpResponse};
use crate::futures::{stream::Stream, Future, IntoFuture};
use crate::network::peer::PeerConnector;
use crate::protos::admin::{
    Circuit, CircuitCreateRequest, CircuitManagementPayload, CircuitManagementPayload_Action,
    CircuitProposal, CircuitProposal_ProposalType,
};
use crate::rest_api::{Method, Resource, RestResourceProvider};
use crate::service::{
    error::{ServiceDestroyError, ServiceError, ServiceStartError, ServiceStopError},
    Service, ServiceMessageContext, ServiceNetworkRegistry, ServiceNetworkSender,
};
use serde_json;

#[derive(Clone)]
pub struct AdminService {
    node_id: String,
    service_id: String,
    network_sender: Option<Box<dyn ServiceNetworkSender>>,
    admin_service_state: Arc<Mutex<AdminServiceState>>,
}

impl AdminService {
    pub fn new(node_id: &str, peer_connector: PeerConnector) -> Self {
        Self {
            node_id: node_id.to_string(),
            service_id: admin_service_id(node_id),
            network_sender: None,
            admin_service_state: Arc::new(Mutex::new(AdminServiceState {
                open_proposals: Default::default(),
                peer_connector,
            })),
        }
    }
}

impl Service for AdminService {
    fn service_id(&self) -> &str {
        &self.service_id
    }

    fn service_type(&self) -> &str {
        "admin"
    }

    fn start(
        &mut self,
        service_registry: &dyn ServiceNetworkRegistry,
    ) -> Result<(), ServiceStartError> {
        let network_sender = service_registry
            .connect(&self.service_id)
            .map_err(|err| ServiceStartError(Box::new(err)))?;

        self.network_sender = Some(network_sender);

        Ok(())
    }

    fn stop(
        &mut self,
        service_registry: &dyn ServiceNetworkRegistry,
    ) -> Result<(), ServiceStopError> {
        service_registry
            .disconnect(&self.service_id)
            .map_err(|err| ServiceStopError(Box::new(err)))?;

        self.network_sender = None;

        Ok(())
    }

    fn destroy(self: Box<Self>) -> Result<(), ServiceDestroyError> {
        Ok(())
    }

    fn handle_message(
        &self,
        message_bytes: &[u8],
        _message_context: &ServiceMessageContext,
    ) -> Result<(), ServiceError> {
        if self.network_sender.is_none() {
            return Err(ServiceError::NotStarted);
        }

        let mut envelope: CircuitManagementPayload = protobuf::parse_from_bytes(message_bytes)
            .map_err(|err| ServiceError::InvalidMessageFormat(Box::new(err)))?;

        match envelope.action {
            CircuitManagementPayload_Action::CIRCUIT_CREATE_REQUEST => {
                let mut create_request = envelope.take_circuit_create_request();

                let proposed_circuit = create_request.take_circuit();
                let mut admin_service_state = self.admin_service_state.lock().map_err(|_| {
                    ServiceError::PoisonedLock("the admin state lock was poisoned".into())
                })?;

                if admin_service_state.has_proposal(proposed_circuit.get_circuit_id()) {
                    info!(
                        "Ignoring duplicate create proposal of circuit {}",
                        proposed_circuit.get_circuit_id()
                    );
                } else {
                    debug!("proposing {}", proposed_circuit.get_circuit_id());

                    let mut proposal = CircuitProposal::new();
                    proposal.set_proposal_type(CircuitProposal_ProposalType::CREATE);
                    proposal.set_circuit_id(proposed_circuit.get_circuit_id().into());
                    proposal.set_circuit_hash(sha256(&proposed_circuit)?);
                    proposal.set_circuit_proposal(proposed_circuit);

                    admin_service_state.add_proposal(proposal);
                }
            }
            unknown_action => {
                error!("Unable to handle {:?}", unknown_action);
            }
        }

        Ok(())
    }
}

impl AdminService {
    /// Propose a new circuit
    ///
    /// This operation will propose a new circuit to all the member nodes of the circuit.  If there
    /// is no peer connection, a connection to the peer will also be established.
    pub fn propose_circuit(&self, proposed_circuit: Circuit) -> Result<(), ServiceError> {
        if self.network_sender.is_none() {
            return Err(ServiceError::NotStarted);
        }

        let mut admin_service_state = self
            .admin_service_state
            .lock()
            .map_err(|_| ServiceError::PoisonedLock("the admin state lock was poisoned".into()))?;

        let mut member_node_ids = vec![];
        for node in proposed_circuit.get_members() {
            if self.node_id != node.get_node_id() {
                admin_service_state
                    .peer_connector
                    .connect_peer(node.get_node_id(), node.get_endpoint())
                    .map_err(|err| ServiceError::UnableToHandleMessage(Box::new(err)))?;

                member_node_ids.push(node.get_node_id().to_string())
            }
        }

        debug!("proposing {}", proposed_circuit.get_circuit_id());

        let mut proposal = CircuitProposal::new();
        proposal.set_proposal_type(CircuitProposal_ProposalType::CREATE);
        proposal.set_circuit_id(proposed_circuit.get_circuit_id().into());
        proposal.set_circuit_hash(sha256(&proposed_circuit)?);
        proposal.set_circuit_proposal(proposed_circuit.clone());

        admin_service_state.add_proposal(proposal);

        let mut create_request = CircuitCreateRequest::new();
        create_request.set_circuit(proposed_circuit);

        let mut envelope = CircuitManagementPayload::new();
        envelope.set_action(CircuitManagementPayload_Action::CIRCUIT_CREATE_REQUEST);
        envelope.set_circuit_create_request(create_request);

        let envelope_bytes = envelope
            .write_to_bytes()
            .map_err(|err| ServiceError::InvalidMessageFormat(Box::new(err)))?;

        for member_id in member_node_ids {
            self.network_sender
                .as_ref()
                .unwrap()
                .send(&admin_service_id(&member_id), &envelope_bytes)?;
        }

        Ok(())
    }
}

fn admin_service_id(node_id: &str) -> String {
    format!("admin::{}", node_id)
}

fn sha256(circuit: &Circuit) -> Result<String, ServiceError> {
    let bytes = circuit
        .write_to_bytes()
        .map_err(|err| ServiceError::UnableToHandleMessage(Box::new(err)))?;
    hash(MessageDigest::sha256(), &bytes)
        .map(|digest| to_hex(&*digest))
        .map_err(|err| ServiceError::UnableToHandleMessage(Box::new(err)))
}

fn to_hex(bytes: &[u8]) -> String {
    let mut buf = String::new();
    for b in bytes {
        write!(&mut buf, "{:0x}", b).expect("Unable to write to string");
    }

    buf
}

struct AdminServiceState {
    open_proposals: HashMap<String, CircuitProposal>,
    peer_connector: PeerConnector,
}

impl AdminServiceState {
    fn add_proposal(&mut self, circuit_proposal: CircuitProposal) {
        let circuit_id = circuit_proposal.get_circuit_id().to_string();

        self.open_proposals.insert(circuit_id, circuit_proposal);
    }

    fn has_proposal(&self, circuit_id: &str) -> bool {
        self.open_proposals.contains_key(circuit_id)
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct CreateCircuit {
    circuit_id: String,
    roster: Vec<SplinterService>,
    members: Vec<SplinterNode>,
    authorization_type: AuthorizationType,
    persistence: PersistenceType,
    routes: RouteType,
    circuit_management_type: String,
    application_metadata: Vec<u8>,
}

impl CreateCircuit {
    fn from_payload(payload: web::Payload) -> impl Future<Item = Self, Error = ActixError> {
        payload
            .from_err()
            .fold(web::BytesMut::new(), move |mut body, chunk| {
                body.extend_from_slice(&chunk);
                Ok::<_, ActixError>(body)
            })
            .and_then(|body| {
                let proposal = serde_json::from_slice::<CreateCircuit>(&body).unwrap();
                Ok(proposal)
            })
            .into_future()
    }
}

#[derive(Serialize, Deserialize, Debug)]
enum AuthorizationType {
    TRUST_AUTHORIZATION,
}

#[derive(Serialize, Deserialize, Debug)]
enum PersistenceType {
    ANY_PERSISTENCE,
}

#[derive(Serialize, Deserialize, Debug)]
enum RouteType {
    ANY_ROUTE,
}

enum ProposalMarshallingError {
    InvalidAuthorizationType,
    InvalidRouteType,
    InvalidPersistenceType,
    InvalidDurabilityType,
    ServiceError(ServiceError),
}

impl From<ServiceError> for ProposalMarshallingError {
    fn from(err: ServiceError) -> Self {
        ProposalMarshallingError::ServiceError(err)
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct SplinterNode {
    node_id: String,
    endpoint: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct SplinterService {
    service_id: String,
    service_type: String,
    allowed_nodes: Vec<String>,
}

impl RestResourceProvider for AdminService {
    fn resources(&self) -> Vec<Resource> {
        vec![
            make_create_circuit_route(),
            make_application_handler_registration_route(),
        ]
    }
}

fn make_create_circuit_route() -> Resource {
    Resource::new(Method::Post, "/auth/circuit", move |r, p| {
        create_circuit(r, p)
    })
}

fn make_application_handler_registration_route() -> Resource {
    Resource::new(Method::Put, "/auth/register/{type}", move |r, _| {
        let circuit_management_type = if let Some(t) = r.match_info().get("type") {
            t
        } else {
            return Box::new(HttpResponse::BadRequest().finish().into_future());
        };

        debug!("circuit management type {}", circuit_management_type);
        Box::new(HttpResponse::Ok().finish().into_future())
    })
}

fn create_circuit(
    req: HttpRequest,
    payload: web::Payload,
) -> Box<Future<Item = HttpResponse, Error = ActixError>> {
    Box::new(CreateCircuit::from_payload(payload).and_then(|circuit| {
        debug!("Circuit: {:#?}", circuit);
        Ok(HttpResponse::Accepted().finish())
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::VecDeque;
    use std::sync::mpsc::{channel, Sender};

    use crate::mesh::Mesh;
    use crate::network::Network;
    use crate::protos::admin;
    use crate::service::{error, ServiceNetworkRegistry, ServiceNetworkSender};
    use crate::transport::{
        ConnectError, Connection, DisconnectError, RecvError, SendError, Transport,
    };

    /// Test that a circuit creation creates the correct connections and sends the appropriate
    /// messages.
    #[test]
    fn test_propose_circuit() {
        let mesh = Mesh::new(4, 16);
        let network = Network::new(mesh.clone());
        let transport =
            MockConnectingTransport::expect_connections(vec![Ok(Box::new(MockConnection))]);

        let peer_connector = PeerConnector::new(network.clone(), Box::new(transport));
        let mut admin_service = AdminService::new("test-node".into(), peer_connector);

        let (tx, rx) = channel();
        admin_service
            .start(&MockNetworkRegistry { tx })
            .expect("Service should have started correctly");

        let mut proposed_circuit = Circuit::new();
        proposed_circuit.set_circuit_id("test_propose_circuit".into());
        proposed_circuit
            .set_authorization_type(admin::Circuit_AuthorizationType::TRUST_AUTHORIZATION);
        proposed_circuit.set_persistence(admin::Circuit_PersistenceType::ANY_PERSISTENCE);
        proposed_circuit.set_routes(admin::Circuit_RouteType::ANY_ROUTE);
        proposed_circuit.set_circuit_management_type("test app auth handler".into());

        proposed_circuit.set_members(protobuf::RepeatedField::from_vec(vec![
            splinter_node("test-node", "tcp://someplace:8000"),
            splinter_node("other-node", "tcp://otherplace:8000"),
        ]));
        proposed_circuit.set_roster(protobuf::RepeatedField::from_vec(vec![
            splinter_service("service-a", "sabre"),
            splinter_service("service-b", "sabre"),
        ]));

        admin_service
            .propose_circuit(proposed_circuit.clone())
            .expect("The proposal was not handled correctly");

        let (recipient, message) = rx.try_recv().expect("A message should have been sent");
        assert_eq!("admin::other-node".to_string(), recipient);

        let mut envelope: CircuitManagementPayload =
            protobuf::parse_from_bytes(&message).expect("The message could not be parsed");
        assert_eq!(
            CircuitManagementPayload_Action::CIRCUIT_CREATE_REQUEST,
            envelope.get_action()
        );
        assert_eq!(
            proposed_circuit,
            envelope.take_circuit_create_request().take_circuit()
        );

        assert_eq!(Some(&"other-node".to_string()), network.peer_ids().get(0));
    }

    fn splinter_node(node_id: &str, endpoint: &str) -> admin::SplinterNode {
        let mut node = admin::SplinterNode::new();
        node.set_node_id(node_id.into());
        node.set_endpoint(endpoint.into());
        node
    }

    fn splinter_service(service_id: &str, service_type: &str) -> admin::SplinterService {
        let mut service = admin::SplinterService::new();
        service.set_service_id(service_id.into());
        service.set_service_type(service_type.into());
        service
    }

    struct MockNetworkRegistry {
        tx: Sender<(String, Vec<u8>)>,
    }

    impl ServiceNetworkRegistry for MockNetworkRegistry {
        fn connect(
            &self,
            _service_id: &str,
        ) -> Result<Box<dyn ServiceNetworkSender>, error::ServiceConnectionError> {
            Ok(Box::new(MockNetworkSender {
                tx: self.tx.clone(),
            }))
        }

        fn disconnect(&self, _service_id: &str) -> Result<(), error::ServiceDisconnectionError> {
            Ok(())
        }
    }

    struct MockNetworkSender {
        tx: Sender<(String, Vec<u8>)>,
    }

    impl ServiceNetworkSender for MockNetworkSender {
        fn send(&self, recipient: &str, message: &[u8]) -> Result<(), error::ServiceSendError> {
            self.tx
                .send((recipient.to_string(), message.to_vec()))
                .expect("Unable to send test message");

            Ok(())
        }

        fn send_and_await(
            &self,
            _recipient: &str,
            _message: &[u8],
        ) -> Result<Vec<u8>, error::ServiceSendError> {
            unimplemented!()
        }

        fn reply(
            &self,
            _message_origin: &ServiceMessageContext,
            _message: &[u8],
        ) -> Result<(), error::ServiceSendError> {
            unimplemented!()
        }

        fn clone_box(&self) -> Box<dyn ServiceNetworkSender> {
            unimplemented!()
        }
    }

    struct MockConnectingTransport {
        connection_results: VecDeque<Result<Box<dyn Connection>, ConnectError>>,
    }

    impl MockConnectingTransport {
        fn expect_connections(results: Vec<Result<Box<dyn Connection>, ConnectError>>) -> Self {
            Self {
                connection_results: results.into_iter().collect(),
            }
        }
    }

    impl Transport for MockConnectingTransport {
        fn accepts(&self, _: &str) -> bool {
            true
        }

        fn connect(&mut self, _: &str) -> Result<Box<dyn Connection>, ConnectError> {
            self.connection_results
                .pop_front()
                .expect("No test result added to mock")
        }

        fn listen(
            &mut self,
            _: &str,
        ) -> Result<Box<dyn crate::transport::Listener>, crate::transport::ListenError> {
            unimplemented!()
        }
    }

    struct MockConnection;

    impl Connection for MockConnection {
        fn send(&mut self, _message: &[u8]) -> Result<(), SendError> {
            Ok(())
        }

        fn recv(&mut self) -> Result<Vec<u8>, RecvError> {
            unimplemented!()
        }

        fn remote_endpoint(&self) -> String {
            String::from("MockConnection")
        }

        fn local_endpoint(&self) -> String {
            String::from("MockConnection")
        }

        fn disconnect(&mut self) -> Result<(), DisconnectError> {
            Ok(())
        }

        fn evented(&self) -> &dyn mio::Evented {
            &MockEvented
        }
    }

    struct MockEvented;

    impl mio::Evented for MockEvented {
        fn register(
            &self,
            _poll: &mio::Poll,
            _token: mio::Token,
            _interest: mio::Ready,
            _opts: mio::PollOpt,
        ) -> std::io::Result<()> {
            Ok(())
        }

        fn reregister(
            &self,
            _poll: &mio::Poll,
            _token: mio::Token,
            _interest: mio::Ready,
            _opts: mio::PollOpt,
        ) -> std::io::Result<()> {
            Ok(())
        }

        fn deregister(&self, _poll: &mio::Poll) -> std::io::Result<()> {
            Ok(())
        }
    }
}