pub mod controls;
pub mod graph_layout;
pub mod main_view;
pub mod modals;
pub mod onboarding;
pub mod version_manager;

/// The lighting recipes live in the vampir toolkit now. Re-exported under
/// the path the view code already uses.
pub use vampir::lighting;
