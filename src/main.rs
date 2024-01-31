use subxt::{OnlineClient, PolkadotConfig};
use subxt_signer::sr25519::PublicKey;
use anyhow::Context as _;
use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::builder::{CreateCommand, CreateInteractionResponse, CreateInteractionResponseMessage};
use serenity::prelude::*;
use serenity::model::id::{GuildId, RoleId};
use serenity::model::application::Interaction;
use shuttle_secrets::SecretStore;
use tracing::{error, info};

#[subxt::subxt(runtime_metadata_path = "kusama-asset-hub-metadata.scale")]
pub mod kusama_asset_hub {}

const GRADUATE_ROLE_ID: u64 = 1202266615509418074;
const CERTIFICATES_COLLECTION: u32 = 15;

struct Bot {
    guild_id: String,
    api: OnlineClient<PolkadotConfig>,
}

#[async_trait]
impl EventHandler for Bot {
    async fn message(&self, ctx: Context, msg: Message) {
        if msg.content == "!hello" {
            if let Err(e) = msg.channel_id.say(&ctx.http, "world!").await {
                error!("Error sending message: {:?}", e);
            }
        }
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        info!("{} is connected!", ready.user.name);

        let guild_id = GuildId::new(self.guild_id.parse().expect("Couldn't parse guild id as a number"));

        let _ = guild_id.set_commands(&ctx.http, vec![
            CreateCommand::new("hello").description("Say hello"),
            CreateCommand::new("checknft").description("Check your NFT certificate"),
        ]).await;
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::Command(command) = interaction {
            info!("Received command interaction: {command:#?}");

            let command_name = command.data.name.as_str();
            let member = command.member.as_ref().expect("Couldn't get member");

            let response_content = match command_name {
                "hello" => "hello".to_owned(),
                "checknft" => {
                    let hex_string = "3283cc9f4408df3ccaef653fc163e56509619c1e0e46bb4e677d227fa50bef7f";
                    let mut bytes = [0u8; 32];
                    hex::decode_to_slice(hex_string, &mut bytes).expect("Couldn't decode hex string");
                    let storage_query = kusama_asset_hub::storage().uniques().account(PublicKey(bytes).to_account_id(), CERTIFICATES_COLLECTION, 22);
                    let result = self.api
                        .storage()
                        .at_latest()
                        .await
                        .expect("Couldn't get storage at latest block") // TODO: Use `?`
                        .fetch(&storage_query)
                        .await
                        .expect("Couldn't fetch storage query");
                    if result.is_some() {
                        member.add_role(&ctx.http, RoleId::new(GRADUATE_ROLE_ID)).await.expect("Couldn't add role to member");
                        "Certificate found! Added role".to_owned()
                    } else {
                        "Certificate not found! Not adding role".to_owned()
                    }
                },
                command => unreachable!("Unknown command {command}"),
            };

            if let Err(why) = command
                .create_response(
                    &ctx.http,
                    CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new().content(response_content),
                    ),
                )
                .await
            {
                error!("Cannot respond to slash command: {why}");
            }
        }
    }
}

#[shuttle_runtime::main]
async fn serenity(
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

    let bot = Bot {
        guild_id,
        api,
    };
    let client = Client::builder(&token, intents)
        .event_handler(bot)
        .await
        .expect("Err creating client");

    Ok(client.into())
}
