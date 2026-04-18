import { TextQuestClient } from "@textquest/client";

async function main(): Promise<void> {
  const client = new TextQuestClient(buildOptions());

  const current = await client.getBoxChatSettings();
  const updated = await client.putBoxChatSettings({
    ...current,
    auto_connect: !current.auto_connect,
  });

  console.log(JSON.stringify(updated, null, 2));
}

void main();

function buildOptions() {
  const apiToken = process.env.TEXTQUEST_API_TOKEN;
  return apiToken
    ? { baseUrl: process.env.TEXTQUEST_BASE_URL ?? "http://127.0.0.1:8080", apiToken }
    : { baseUrl: process.env.TEXTQUEST_BASE_URL ?? "http://127.0.0.1:8080" };
}
