#![forbid(unsafe_code)]
//! The AI behind Jarvis: one conversation sent to a model and the answer read back as it is written. Two wire
//! formats cover the sources there are: Anthropic's Messages API, and the OpenAI chat format that OpenAI, Gemini,
//! DeepSeek and a model on this machine (Ollama) all speak. The request goes through the system's curl with its
//! arguments on standard input, so the key never appears in the list of processes.

pub mod chat;
pub mod codex;
pub mod wire;

pub use chat::{Cancel, Failure, Source, stream};
pub use wire::{Kind, Message, Role};
