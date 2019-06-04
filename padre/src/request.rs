#[derive(Clone, Deserialize, Debug, PartialEq)]
pub enum PadreRequestCmd {
    Cmd(String),
    CmdWithFileLocation(String, String, u64),
    CmdWithVariable(String, String),
}

#[derive(Deserialize, Debug, PartialEq)]
pub struct PadreRequest {
    id: u64,
    cmd: PadreRequestCmd,
}

impl PadreRequest {
    pub fn new(id: u64, cmd: PadreRequestCmd) -> Self {
        PadreRequest { id, cmd }
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn cmd(&self) -> &PadreRequestCmd {
        &self.cmd
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub enum PadreResponse {
    Response(u64, serde_json::Value),
    Notify(String, Vec<serde_json::Value>),
}
