/// Dialog rendering modules.
/// Each file renders one or more Dialog variants as a free function that
/// receives the destructured dialog fields + ThemeColors — no &App required.

pub mod changelog;
pub mod dll_warning;
pub mod mod_file_info;
pub mod remove_repo;
pub mod simple_warnings;
pub mod instance;
// TODO: pub mod add_repo; — deeply coupled to App state; extract in a follow-up pass
