#[derive(Debug, Clone, Copy)]
pub struct RasaVariables {
    pub show_box: bool,
    // Control the number of seconds the graphs look backward
    pub look_behind: usize,
    // Temporary, number of points to skip for the LSTM optimization purposes (expands the size of the box)
    // TODO: Replace with autosizing the box based on float time
    pub skip: usize,
    pub channels: usize,
}