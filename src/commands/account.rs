use {
    crate::{
        commands::CommandFlow,
        context::ScillaContext,
        misc::helpers::{bincode_deserialize, lamports_to_sol, sol_to_lamports},
        prompt::prompt_input_data,
        ui::{print_error, show_spinner},
        constants::{LAMPORTS_PER_SOL, MAX_TRANSFER_SOL},
    },
    anyhow::bail,
    comfy_table::{Cell, Table, presets::UTF8_FULL},
    console::style,
    inquire::Select,
    solana_nonce::versions::Versions,
    solana_pubkey::Pubkey,
    solana_rpc_client_api::config::{RpcLargestAccountsConfig, RpcLargestAccountsFilter},
    std::fmt,
    solana_signature::Signature,
    solana_system_interface::instruction::transfer,
    solana_transaction::Transaction,
    solana_message::Message,
};



/// Commands related to wallet or account management
#[derive(Debug, Clone)]
pub enum AccountCommand {
    FetchAccount,
    Balance,
    Transfer,
    Airdrop,
    LargestAccounts,
    NonceAccount,
    GoBack,
}

impl AccountCommand {
    pub fn spinner_msg(&self) -> &'static str {
        match self {
            AccountCommand::FetchAccount => "Fetching account…",
            AccountCommand::Balance => "Checking SOL balance…",
            AccountCommand::Transfer => "Sending SOL…",
            AccountCommand::Airdrop => "Requesting SOL on devnet/testnet…",
            AccountCommand::LargestAccounts => "Fetching largest accounts on the cluster…",
            AccountCommand::NonceAccount => "Inspecting or managing durable nonces…",
            AccountCommand::GoBack => "Going back…",
        }
    }
}

impl fmt::Display for AccountCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let command = match self {
            AccountCommand::FetchAccount => "Fetch account",
            AccountCommand::Balance => "Check balance",
            AccountCommand::Transfer => "Transfer SOL",
            AccountCommand::Airdrop => "Request airdrop",
            AccountCommand::LargestAccounts => "View largest accounts",
            AccountCommand::NonceAccount => "View nonce account",
            AccountCommand::GoBack => "Go back",
        };
        write!(f, "{command}")
    }
}

impl AccountCommand {
    pub async fn process_command(&self, ctx: &ScillaContext) -> CommandFlow<()> {
        match self {
            AccountCommand::FetchAccount => {
                let pubkey: Pubkey = prompt_input_data("Enter Pubkey:");
                show_spinner(self.spinner_msg(), fetch_acc_data(ctx, &pubkey)).await;
            }
            AccountCommand::Balance => {
                let pubkey: Pubkey = prompt_input_data("Enter Pubkey :");
                show_spinner(self.spinner_msg(), fetch_account_balance(ctx, &pubkey)).await;
            }
            AccountCommand::Transfer => {
                let to: Pubkey = prompt_input_data("Enter recipient Pubkey:");
                let amount: f64 = prompt_input_data("Enter amount (SOL):");
                if amount > MAX_TRANSFER_SOL {
                    print_error(format!("Amount exceeds maximum allowed limit of {} SOL", MAX_TRANSFER_SOL));
                } else {
                    let result = show_spinner(self.spinner_msg(), transfer_sol(ctx, to, amount)).await;
                    // show_spinner returns (), it prints errors internally if needed?
                    // Wait, check ui.rs: show_spinner returns nothing, but it processes Result inside.
                    // The old code had: let res = show_spinner(...); if let Err(e) == res ...
                    // ui.rs definition: pub async fn show_spinner<F, T>(message: &str, fut: F)
                    // It does NOT return the result of the future. It swallows it and prints "Done" or "Error".
                    // So we don't need to check result here if show_spinner handles printing.
                    // The incoming implementation tried to check result?
                    // Incoming: let res = show_spinner(...); ...
                    // HEAD implementation of show_spinner returns (), so incoming call was likely wrong for this version of show_spinner?
                    // No, look closely at ui.rs: `pub async fn show_spinner<F, T>(...)` no return type specified -> returns ().
                    // So `let res = show_spinner(...)` would be unit `()`.
                    // So checking `if let Err(e) = res` would fail compilation.
                    // So we just call it.
                }
            }
            AccountCommand::Airdrop => {
                show_spinner(self.spinner_msg(), request_sol_airdrop(ctx)).await;
            }
            AccountCommand::LargestAccounts => {
                show_spinner(self.spinner_msg(), fetch_largest_accounts(ctx)).await;
            }
            AccountCommand::NonceAccount => {
                let pubkey: Pubkey = prompt_input_data("Enter nonce account pubkey:");
                show_spinner(self.spinner_msg(), fetch_nonce_account(ctx, &pubkey)).await;
            }
            AccountCommand::GoBack => {
                return CommandFlow::GoBack;
            }
        }

        CommandFlow::Process(())
    }
}

async fn request_sol_airdrop(ctx: &ScillaContext) -> anyhow::Result<()> {
    // request an airdrop worth of 1 SOL
    let sig = ctx
        .rpc()
        .request_airdrop(ctx.pubkey(), sol_to_lamports(1.0))
        .await;
    match sig {
        Ok(signature) => {
            println!(
                "{} {}",
                style("Airdrop requested successfully!").green().bold(),
                style(format!("Signature: {signature}")).cyan()
            );
        }
        Err(err) => {
            print_error(format!("Airdrop failed: {err}"));
        }
    }

    Ok(())
}

async fn fetch_acc_data(ctx: &ScillaContext, pubkey: &Pubkey) -> anyhow::Result<()> {
    let acc = ctx.rpc().get_account(pubkey).await?;

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_header(vec![
            Cell::new("Field")
                .add_attribute(comfy_table::Attribute::Bold)
                .fg(comfy_table::Color::Cyan),
            Cell::new("Value")
                .add_attribute(comfy_table::Attribute::Bold)
                .fg(comfy_table::Color::Cyan),
        ])
        .add_row(vec![
            Cell::new("Lamports"),
            Cell::new(format!("{}", acc.lamports)),
        ])
        .add_row(vec![
            Cell::new("Data Length"),
            Cell::new(format!("{}", acc.data.len())),
        ])
        .add_row(vec![
            Cell::new("Owner"),
            Cell::new(format!("{}", acc.owner)),
        ])
        .add_row(vec![
            Cell::new("Executable"),
            Cell::new(format!("{}", acc.executable)),
        ])
        .add_row(vec![
            Cell::new("Rent Epoch"),
            Cell::new(format!("{}", acc.rent_epoch)),
        ]);

    println!("{}\n{}", style("ACCOUNT INFO").green().bold(), table);

    Ok(())
}

async fn fetch_account_balance(ctx: &ScillaContext, pubkey: &Pubkey) -> anyhow::Result<()> {
    let acc = ctx.rpc().get_account(pubkey).await?;
    let acc_balance = lamports_to_sol(acc.lamports);

    println!(
        "{} {}",
        style("Account balance in SOL:").green().bold(),
        style(format!("{acc_balance:#?}")).cyan()
    );

    Ok(())
}

async fn fetch_largest_accounts(ctx: &ScillaContext) -> anyhow::Result<()> {
    let filter_choice = Select::new(
        "Filter accounts by:",
        vec!["All", "Circulating", "Non-Circulating"],
    )
    .prompt()?;

    let filter = match filter_choice {
        "Circulating" => Some(RpcLargestAccountsFilter::Circulating),
        "Non-Circulating" => Some(RpcLargestAccountsFilter::NonCirculating),
        _ => None,
    };

    let config = RpcLargestAccountsConfig {
        commitment: Some(ctx.rpc().commitment()),
        filter,
        sort_results: Some(true),
    };

    let response = ctx.rpc().get_largest_accounts_with_config(config).await?;
    let largest_accounts = response.value;

    let mut table = Table::new();
    table.load_preset(UTF8_FULL).set_header(vec![
        Cell::new("#").add_attribute(comfy_table::Attribute::Bold),
        Cell::new("Address").add_attribute(comfy_table::Attribute::Bold),
        Cell::new("Balance (SOL)").add_attribute(comfy_table::Attribute::Bold),
    ]);

    for (idx, account) in largest_accounts.iter().enumerate() {
        let balance_sol = lamports_to_sol(account.lamports);
        table.add_row(vec![
            Cell::new(format!("{}", idx + 1)),
            Cell::new(&account.address),
            Cell::new(format!("{balance_sol:.2}")),
        ]);
    }

    println!("\n{}", style("LARGEST ACCOUNTS").green().bold());
    println!("{table}");

    Ok(())
}

async fn fetch_nonce_account(ctx: &ScillaContext, pubkey: &Pubkey) -> anyhow::Result<()> {
    let account = ctx.rpc().get_account(pubkey).await?;

    let versions = bincode_deserialize::<Versions>(&account.data, "nonce account data")?;

    let solana_nonce::state::State::Initialized(data) = versions.state() else {
        bail!("This account is not an initialized nonce account");
    };

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_header(vec![
            Cell::new("Field")
                .add_attribute(comfy_table::Attribute::Bold)
                .fg(comfy_table::Color::Cyan),
            Cell::new("Value")
                .add_attribute(comfy_table::Attribute::Bold)
                .fg(comfy_table::Color::Cyan),
        ])
        .add_row(vec![Cell::new("Address"), Cell::new(pubkey)])
        .add_row(vec![
            Cell::new("Lamports"),
            Cell::new(format!("{}", account.lamports)),
        ])
        .add_row(vec![
            Cell::new("Balance (SOL)"),
            Cell::new(format!("{:.6}", lamports_to_sol(account.lamports))),
        ])
        .add_row(vec![Cell::new("Owner"), Cell::new(account.owner)])
        .add_row(vec![
            Cell::new("Executable"),
            Cell::new(format!("{}", account.executable)),
        ])
        .add_row(vec![
            Cell::new("Rent Epoch"),
            Cell::new(format!("{}", account.rent_epoch)),
        ])
        .add_row(vec![
            Cell::new("Nonce blockhash"),
            Cell::new(data.blockhash()),
        ])
        .add_row(vec![Cell::new("Authority"), Cell::new(data.authority)]);

    println!("\n{}", style("NONCE ACCOUNT INFO").green().bold());
    println!("{table}");

    Ok(())
}

async fn transfer_sol(ctx: &ScillaContext, to: Pubkey, amount_sol: f64) -> anyhow::Result<()> {
    let lamports = (amount_sol * LAMPORTS_PER_SOL as f64) as u64;
    let from_pubkey = ctx.pubkey();
    
    let instruction = transfer(&from_pubkey, &to, lamports);
    let recent_blockhash = ctx.rpc().get_latest_blockhash().await?;
    
    let message = Message::new(&[instruction], Some(from_pubkey));
    let transaction = Transaction::new(&[ctx.keypair()], message, recent_blockhash);
    
    let signature = ctx.rpc().send_and_confirm_transaction(&transaction).await?;
    
    println!(
        "\n{} {}\n{}",
        style("Transfer successful!").green().bold(),
        style(format!("Amount: {} SOL", amount_sol)).cyan(),
        style(format!("Signature: {}", signature)).yellow()
    );

    Ok(())
}
