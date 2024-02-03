use subxt::{OnlineClient, PolkadotConfig};
use subxt_signer::sr25519::PublicKey;
use anyhow::Context as _;
use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::builder::{CreateCommand, CreateCommandOption, CreateInteractionResponse, CreateInteractionResponseMessage};
use serenity::all::CommandOptionType;
use serenity::prelude::*;
use serenity::model::id::{GuildId, RoleId};
use serenity::model::application::Interaction;
use shuttle_secrets::SecretStore;
use tracing::{error, info};
use poise::serenity_prelude::Member;

#[subxt::subxt(runtime_metadata_path = "kusama-asset-hub-metadata.scale")]
pub mod kusama_asset_hub {}

const GRADUATE_ROLE_ID: u64 = 1202266615509418074;
const CERTIFICATES_COLLECTION: u32 = 15;

struct Data {
    api: OnlineClient<PolkadotConfig>,
}
type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;

#[poise::command(slash_command)]
async fn hello(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say("world!").await?;
    Ok(())
}

#[poise::command(slash_command)]
async fn checkcertificate(ctx: Context<'_>, #[description = "Polkadot address public key (hex)"] address: String) -> Result<(), Error> {
    let mut bytes = [0u8; 32];
    hex::decode_to_slice(address, &mut bytes).map_err(|_| "Invalid address")?;
    let storage_query = kusama_asset_hub::storage().uniques().account(PublicKey(bytes).to_account_id(), CERTIFICATES_COLLECTION, 22);
    let result = ctx.data().api
        .storage()
        .at_latest()
        .await
        .map_err(|error| {
            error!("Couldn't get storage at latest block: {error:?}");
            "Internal error"
        })?
        .fetch(&storage_query)
        .await
        .map_err(|error| {
            error!("Couldn't fetch storage query: {error:?}");
            "Internal error"
        })?;
    if result.is_some() {
        let member = ctx.author_member().await.ok_or_else(|| {
            error!("Couldn't get member");
            "Internal error"
        })?;
        member.add_role(&ctx.http(), RoleId::new(GRADUATE_ROLE_ID)).await.map_err(|error| {
            error!("Couldn't add role to member: {error:?}");
            "Internal error"
        })?;
        ctx.say("Certificate found! Added role").await?;
        Ok(())
    } else {
        ctx.say("Certificate not found. Not adding role").await?;
        Ok(())
    }
}

#[shuttle_runtime::main]
async fn main(
    #[shuttle_secrets::Secrets] secret_store: SecretStore,
) -> shuttle_serenity::ShuttleSerenity {
    // Get secrets
    let token = secret_store.get("DISCORD_TOKEN").context("'DISCORD_TOKEN' was not found")?;
    let guild_id = secret_store.get("GUILD_ID").context("'GUILD_ID' was not found")?;

    // Set gateway intents, which decides what events the bot will be notified about
    let intents = GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT;

    // Connect to Kusama
    let api = OnlineClient::<PolkadotConfig>::from_url("wss://rpc-asset-hub-kusama.luckyfriday.io")
        .await
        .expect("Couldn't connect to RPC node"); // TODO: Use `?`

    // Setup `poise` framework
    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![hello(), checkcertificate()],
            ..Default::default()
        })
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                Ok(Data {
                    api,
                })
            })
        })
        .build();

    let client = Client::builder(&token, intents)
        .framework(framework)
        .await
        .map_err(shuttle_runtime::CustomError::new)?;

    Ok(client.into())
}
