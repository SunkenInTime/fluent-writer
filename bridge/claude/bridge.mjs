// Fluent Writer <-> Claude Agent SDK bridge.
// Reads one {"type":"request"} line on stdin, streams {"type":"delta"} lines,
// ends with {"type":"done"} or {"type":"error"}. Every tool request is denied.
import { createInterface } from "node:readline";

const write = (msg) => process.stdout.write(JSON.stringify(msg) + "\n");
const abort = new AbortController();
let activeQuery = null;
let finished = false;

const finish = (msg) => {
  if (finished) return;
  finished = true;
  write(msg);
  setTimeout(() => process.exit(0), 50);
};

const friendly = (message) => {
  const lower = String(message).toLowerCase();
  if (lower.includes("login") || lower.includes("auth") || lower.includes("api key") || lower.includes("401")) {
    return "Claude isn't signed in. Run `claude` in Terminal and sign in, then try again.";
  }
  if (lower.includes("enoent") || lower.includes("native binary") || lower.includes("executable")) {
    return "Claude Code isn't installed. Install it with `npm install -g @anthropic-ai/claude-code`.";
  }
  return String(message);
};

async function run(request) {
  let sdk;
  try {
    sdk = await import("@anthropic-ai/claude-agent-sdk");
  } catch {
    finish({ type: "error", message: "The Claude bridge dependencies are missing. Run `npm install` in bridge/claude." });
    return;
  }
  const options = {
    cwd: request.cwd,
    abortController: abort,
    includePartialMessages: true,
    maxTurns: 1,
    permissionMode: "default",
    settingSources: [],
    tools: [],
    allowedTools: [],
    disallowedTools: [
      "Bash", "BashOutput", "KillShell", "Read", "Write", "Edit", "MultiEdit", "NotebookEdit",
      "Glob", "Grep", "LS", "WebFetch", "WebSearch", "Task", "TodoWrite", "ExitPlanMode",
    ],
    mcpServers: {},
    canUseTool: async () => ({ behavior: "deny", message: "Writing assistance has no tools.", interrupt: true }),
    systemPrompt: "You are a careful writing editor. You only read the text you are given and reply in the requested format.",
  };
  if (process.env.FLUENT_WRITER_CLAUDE_PATH) options.pathToClaudeCodeExecutable = process.env.FLUENT_WRITER_CLAUDE_PATH;

  let text = "";
  try {
    activeQuery = sdk.query({ prompt: request.prompt, options });
    for await (const message of activeQuery) {
      if (finished) return;
      if (message.type === "stream_event") {
        const event = message.event;
        if (event?.type === "content_block_delta" && event.delta?.type === "text_delta") {
          text += event.delta.text;
          write({ type: "delta", text: event.delta.text });
        }
      } else if (message.type === "assistant" && !text) {
        const blocks = message.message?.content ?? [];
        const t = blocks.filter((b) => b.type === "text").map((b) => b.text).join("");
        if (t) {
          text = t;
          write({ type: "delta", text: t });
        }
      } else if (message.type === "result") {
        if (message.subtype === "success") {
          const out = typeof message.result === "string" && message.result ? message.result : text;
          if (/^not logged in/i.test(out.trim())) { finish({ type: "error", message: friendly("login") }); return; }
          finish({ type: "done", text: typeof message.result === "string" && message.result ? message.result : text });
        } else {
          finish({ type: "error", message: friendly(message.errors?.join(" ") || message.subtype) });
        }
        return;
      }
    }
    finish({ type: "done", text });
  } catch (error) {
    finish({ type: "error", message: abort.signal.aborted ? "Stopped." : friendly(error?.message ?? error) });
  }
}

const cancel = () => {
  abort.abort();
  activeQuery?.interrupt?.().catch(() => {});
  finished = true;
  setTimeout(() => process.exit(0), 100);
};

process.on("SIGTERM", cancel);
const rl = createInterface({ input: process.stdin });
rl.on("line", (line) => {
  let msg;
  try { msg = JSON.parse(line); } catch { return; }
  if (msg.type === "request") run(msg);
  else if (msg.type === "cancel") cancel();
});
rl.on("close", () => { if (!finished) cancel(); });
