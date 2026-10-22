// figghost-pi-extension
// Installed by Figghost from its settings (AI agents, Pi), which also removes it; Figghost rewrites this file
// when it starts, so changes made here do not last.
// It tells the Figghost notch what pi does, and lets a question of ask_user_question be answered there too.
// Nothing here may get in pi's way: every call is bounded, every error is swallowed and nothing is printed.
// @ts-nocheck

import net from "node:net";

// Written in by Figghost when it installs the file.
const SOCKET = "__FIGGHOST_SOCKET__";
// The app answers every event at once; this only bounds an app that hangs.
const ACK_MS = 1500;
const CONNECT_MS = 500;
// What the notch can show, as in core-agents event.rs.
const QUESTIONS_MAX = 4;
const OPTIONS_MAX = 4;

function clip(text, max) {
  const chars = Array.from(String(text ?? "").trim());
  return chars.length > max ? chars.slice(0, max).join("") + "..." : chars.join("");
}

// The questions as the notch shows them, or none when it could not answer them whole: an answer has to
// cover every question, and the indexes it gives must point at the options pi has.
function questionsOf(data) {
  const list = Array.isArray(data?.questions) ? data.questions : [];
  if (list.length === 0 || list.length > QUESTIONS_MAX) return [];
  const questions = list.map((q) => ({
    header: clip(q?.header, 30),
    question: clip(q?.question, 300),
    options: (Array.isArray(q?.options) ? q.options : [])
      .slice(0, OPTIONS_MAX)
      .map((o, n) => clip(o?.label, 80) || String(n + 1)),
    multi: q?.multiSelect === true,
  }));
  return questions.every((q) => q.question && q.options.length > 0) ? questions : [];
}

export default function (pi) {
  let session; // { id, cwd } of the terminal session, once known
  const asking = new Map(); // toolCallId -> the open connection that waits for the notch's answer
  let queue = Promise.resolve();

  function line(kind, more = {}) {
    return JSON.stringify({ agent: "pi", kind, session_id: session.id, cwd: session.cwd, at_ms: Date.now(), ...more }) + "\n";
  }

  // One event, acknowledged by the app; resolves when that is over, however it went.
  function send(text) {
    return new Promise((resolve) => {
      let socket;
      const done = () => {
        clearTimeout(timer);
        socket?.destroy();
        resolve();
      };
      const timer = setTimeout(done, ACK_MS);
      try {
        socket = net.createConnection(SOCKET);
        socket.on("error", done);
        socket.on("close", done);
        socket.on("data", done);
        socket.on("connect", () => socket.write(text));
        socket.unref?.();
      } catch {
        done();
      }
    });
  }

  // In order, so the app never sees a turn end before it began.
  function report(kind) {
    if (!session) return Promise.resolve();
    const text = line(kind);
    queue = queue.then(() => send(text));
    return queue;
  }

  function remember(ctx) {
    try {
      const id = ctx?.sessionManager?.getSessionId?.();
      if (typeof id === "string" && id) session = { id, cwd: String(ctx?.cwd ?? "") };
    } catch {
      // No session id: nothing is reported.
    }
  }

  function ask(data) {
    const toolCallId = data?.toolCallId;
    const questions = questionsOf(data);
    if (!session || typeof toolCallId !== "string" || asking.has(toolCallId) || questions.length === 0) return;
    const text = line("question", { tool: "ask_user_question", detail: questions[0].question, questions });
    const socket = net.createConnection(SOCKET);
    asking.set(toolCallId, socket);
    let connected = false;
    let buffer = "";
    const timer = setTimeout(() => connected || socket.destroy(), CONNECT_MS);
    socket.setEncoding("utf8");
    socket.on("error", () => {});
    socket.on("close", () => {
      clearTimeout(timer);
      if (asking.get(toolCallId) === socket) asking.delete(toolCallId);
    });
    socket.on("connect", () => {
      connected = true;
      socket.write(text);
    });
    socket.on("data", (chunk) => {
      buffer += chunk;
      const end = buffer.indexOf("\n");
      if (end < 0) return;
      socket.destroy();
      let reply;
      try {
        reply = JSON.parse(buffer.slice(0, end));
      } catch {
        return;
      }
      // A pass means the notch let it go: the terminal keeps asking.
      const indexes = reply?.decision === "answers" ? reply.indexes : undefined;
      if (!Array.isArray(indexes) || indexes.length !== questions.length) return;
      pi.events.emit("rpiv:ask-user:answer", {
        toolCallId,
        answers: indexes.map((optionIndexes, questionIndex) => ({ questionIndex, optionIndexes })),
      });
    });
    // A question that waits must never keep pi from quitting.
    socket.unref?.();
  }

  const guard = (fn) => (...args) => {
    try {
      return fn(...args);
    } catch {
      return undefined;
    }
  };

  // Only the terminal session a person sees: subagents and print runs are not sessions of theirs.
  const shown = (ctx) => ctx?.mode === "tui";

  pi.on("session_start", guard((_event, ctx) => {
    if (!shown(ctx)) return;
    remember(ctx);
    void report("session_start");
  }));
  pi.on("agent_start", guard((_event, ctx) => {
    if (!shown(ctx)) return;
    remember(ctx);
    void report("prompt_submit");
  }));
  // agent_end can come several times for one prompt (retries, compaction, queued messages); this comes once.
  pi.on("agent_settled", guard((event, ctx) => {
    if (!shown(ctx)) return;
    void report(event?.aborted ? "interrupt" : "stop");
  }));
  pi.on("session_shutdown", guard(async () => {
    for (const socket of asking.values()) socket.destroy();
    asking.clear();
    await report("session_end");
    session = undefined;
  }));
  pi.events.on("rpiv:ask-user:prompt", guard(ask));
  pi.events.on("rpiv:ask-user:blocked", guard((data) => {
    // However it was answered, the questionnaire is closed: hanging up takes the card off the notch.
    if (data?.active === false && typeof data.toolCallId === "string") asking.get(data.toolCallId)?.destroy();
  }));
}
