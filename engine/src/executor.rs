use crate::engine::ArbitrageCycle;

pub fn execute_arbitrage(cycles: &Vec<ArbitrageCycle>) {
       println!("Executing arbitrage for {} cycles", cycles.len());
       //sort cycles with a sanity check on profit_pct
         let mut sorted_cycles = cycles.clone();
            sorted_cycles.retain(|c| c.profit_pct > 0.5); // Keep only cycles with profit_pct > 0.5%
}

