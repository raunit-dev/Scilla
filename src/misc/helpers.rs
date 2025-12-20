use {
    crate::{constants::LAMPORTS_PER_SOL, context::ScillaContext},
    solana_instruction::Instruction,
    solana_message::Message,
    solana_signature::Signature,
    solana_transaction::Transaction,
};

pub fn lamports_to_sol(lamports: u64) -> f64 {
    lamports as f64 / LAMPORTS_PER_SOL as f64
}

pub fn sol_to_lamports(sol: f64) -> u64 {
    (sol * LAMPORTS_PER_SOL as f64) as u64
}

pub async fn build_and_send_tx(
    ctx: &ScillaContext,
    instructions: &[Instruction],
) -> anyhow::Result<Signature> {
    let recent_blockhash = ctx.rpc().get_latest_blockhash().await?;
    let message = Message::new(instructions, Some(ctx.pubkey()));
    let transaction = Transaction::new(&[ctx.keypair()], message, recent_blockhash);
    let signature = ctx.rpc().send_and_confirm_transaction(&transaction).await?;
    Ok(signature)
}
