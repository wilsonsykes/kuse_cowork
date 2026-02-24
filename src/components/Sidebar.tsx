import { Component, For } from "solid-js";
import { useChat } from "../stores/chat";
import { useAppVersion } from "../lib/app-version";
import "./Sidebar.css";

interface SidebarProps {
  onSettingsClick: () => void;
  mode: "chat" | "agent";
  onModeChange: (mode: "chat" | "agent") => void;
}

const Sidebar: Component<SidebarProps> = (props) => {
  const appVersion = useAppVersion();

  const {
    conversations,
    activeConversationId,
    selectConversation,
    createConversation,
    deleteConversation,
  } = useChat();

  return (
    <aside class="sidebar">
      <div class="sidebar-header">
        <div class="logo-container">
          <img src="/logo.png" alt="Kuse Cowork" class="logo-image" />
          <div class="brand-block">
            <h1 class="logo-text">Kuse Cowork</h1>
            <div class="brand-meta">
              <p class="logo-subtitle">by Wilson</p>
              <span class="version-tag">{appVersion()}</span>
            </div>
          </div>
        </div>
        <div class="mode-tabs">
          <button
            class={`mode-tab ${props.mode === "chat" ? "active" : ""}`}
            onClick={() => props.onModeChange("chat")}
          >
            Chat
          </button>
          <button
            class={`mode-tab ${props.mode === "agent" ? "active" : ""}`}
            onClick={() => props.onModeChange("agent")}
          >
            Agent
          </button>
        </div>
        <button class="new-chat-btn" onClick={() => createConversation()}>
          + New Chat
        </button>
      </div>

      <nav class="conversations">
        <For each={conversations()}>
          {(conv) => (
            <div
              class={`conversation-item ${
                conv.id === activeConversationId() ? "active" : ""
              }`}
              onClick={() => selectConversation(conv.id)}
            >
              <span class="conversation-title">{conv.title}</span>
              <button
                class="delete-btn"
                onClick={(e) => {
                  e.stopPropagation();
                  deleteConversation(conv.id);
                }}
              >
                x
              </button>
            </div>
          )}
        </For>
      </nav>

      <div class="sidebar-footer">
        <button
          class="footer-btn primary-btn"
          onClick={() => {
            // TODO: Implement Skills Manager functionality
            console.log("Skills Manager clicked - coming soon");
          }}
        >
          Skills
        </button>
        <button
          class="footer-btn primary-btn"
          onClick={() => {
            // TODO: Implement MCPs functionality
            console.log("MCPs clicked - coming soon");
          }}
        >
          MCPs
        </button>
        <button class="footer-btn primary-btn" onClick={props.onSettingsClick}>
          Settings
        </button>
      </div>
    </aside>
  );
};

export default Sidebar;
