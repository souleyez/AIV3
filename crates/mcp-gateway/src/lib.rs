#[derive(Clone, Debug)]
pub struct RemoteServerRegistration {
    pub name: String,
    pub transport: String,
    pub endpoint: String,
}

#[derive(Default)]
pub struct McpGateway {
    servers: Vec<RemoteServerRegistration>,
}

impl McpGateway {
    pub fn register_server(&mut self, server: RemoteServerRegistration) {
        self.servers.push(server);
    }

    pub fn servers(&self) -> &[RemoteServerRegistration] {
        &self.servers
    }
}
