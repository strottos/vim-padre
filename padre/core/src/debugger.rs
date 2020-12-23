#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileLocation {
    name: String,
    line_num: u64,
}

impl FileLocation {
    pub fn new(name: String, line_num: u64) -> Self {
        FileLocation { name, line_num }
    }

    pub fn name(&self) -> &str {
        &self.name[..]
    }

    pub fn line_num(&self) -> u64 {
        self.line_num
    }
}

/// Variable name
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Variable {
    name: String,
}

impl Variable {
    pub fn new(name: String) -> Self {
        Variable { name }
    }

    pub fn name(&self) -> &str {
        &self.name[..]
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum DebuggerCmd {
    Run,
    Breakpoint(FileLocation),
    Unbreakpoint(FileLocation),
    // Count
    StepIn(u64),
    // Count
    StepOver(u64),
    Continue,
    Print(Variable),
}
