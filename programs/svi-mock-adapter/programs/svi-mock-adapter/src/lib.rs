pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;
use svi_core::{cpi::accounts::PublishQuote, program::SviCore, QuoteUpdate};

pub use constants::*;
pub use instructions::*;
pub use state::*;

declare_id!("3kK7593G5wN5R9S9Sce5zVAy2BuijtMEFwzUCjh3oW8M");

#[program]
pub mod svi_mock_adapter {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        crate::instructions::initialize::handle_initialize(ctx)
    }

    pub fn increment(ctx: Context<Increment>) -> Result<()> {
        crate::instructions::increment::handle_increment(ctx)
    }
}
