#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Filter {
    #[default]
    All,
    Updates,
    Errors,
    Ignored,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortKey {
    #[default]
    Name,
    Status,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortDir {
    #[default]
    Asc,
    Desc,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LogFilter {
    #[default]
    All,
    Info,
    Errors,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Error,
}

#[derive(Debug, Clone)]
pub struct LogLine {
    pub level: LogLevel,
    pub text: String,
    pub timestamp: String,
}

#[derive(Debug, Clone)]
pub enum InstanceField {
    Name(String),
    WowDir(String),
    LaunchMethod(String),
    LikeTurtles(bool),
    ClearWdb(bool),
    LutrisTarget(String),
    WineCommand(String),
    WineArgs(String),
    CustomCommand(String),
    CustomArgs(String),
}
