//! Wire types, one module per API. Names are prefixed per module so the glob re-exports below
//! never collide: Gamma keeps its original names (`Polymarket*`, `EventsPage`, ...) and adds
//! `Gamma*`; CLOB uses `Clob*`; Data API uses `Data*`; the market websocket uses `MarketWs*`.

pub(crate) mod de;

pub mod clob;
pub mod data;
pub mod gamma;
pub mod ws;

pub use clob::*;
pub use data::*;
pub use gamma::*;
pub use ws::*;
