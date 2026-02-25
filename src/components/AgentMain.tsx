import { Component, Show, For, createSignal } from "solid-js";
import { Task, TaskMessage, openMultipleFoldersDialog, openImageFilesDialog } from "../lib/tauri-api";
import { useSettings } from "../stores/settings";
import { isLikelyVisionModel } from "../stores/settings";
import "./AgentMain.css";

interface AgentMainProps {
  onNewTask: (
    title: string,
    description: string,
    projectPath?: string,
    imagePaths?: string[],
    imageData?: Array<{ name?: string; media_type: string; data: string }>
  ) => void;
  onContinueTask: (
    message: string,
    projectPath?: string,
    imagePaths?: string[],
    imageData?: Array<{ name?: string; media_type: string; data: string }>
  ) => void;
  onNewConversation: () => void;
  currentText: string;
  isRunning: boolean;
  activeTask: Task | null;
  messages: TaskMessage[];
}

const AgentMain: Component<AgentMainProps> = (props) => {
  const { isConfigured, toggleSettings, settings } = useSettings();
  const [input, setInput] = createSignal("");
  const [selectedPaths, setSelectedPaths] = createSignal<string[]>([]);
  const [selectedImages, setSelectedImages] = createSignal<string[]>([]);
  const [pastedImages, setPastedImages] = createSignal<Array<{ id: string; name: string; mediaType: string; data: string }>>([]);
  const [showPathsPanel, setShowPathsPanel] = createSignal(false);
  const [showImagesPanel, setShowImagesPanel] = createSignal(false);

  // Check if we're in an existing conversation
  const isInConversation = () => props.activeTask !== null && props.messages.length > 0;

  const handleAddFolders = async () => {
    const folders = await openMultipleFoldersDialog();
    if (folders.length > 0) {
      // Add new folders (avoid duplicates)
      const existing = selectedPaths();
      const newPaths = folders.filter(f => !existing.includes(f));
      setSelectedPaths([...existing, ...newPaths]);
      setShowPathsPanel(true);
    }
  };

  const handleRemovePath = (path: string) => {
    setSelectedPaths(selectedPaths().filter(p => p !== path));
  };

  const handleAddImages = async () => {
    const files = await openImageFilesDialog();
    if (files.length > 0) {
      const existing = selectedImages();
      const newFiles = files.filter(f => !existing.includes(f));
      setSelectedImages([...existing, ...newFiles]);
      setShowImagesPanel(true);
    }
  };

  const handleRemoveImage = (path: string) => {
    setSelectedImages(selectedImages().filter(p => p !== path));
  };

  const toBase64 = (file: File): Promise<string> => new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const result = String(reader.result || "");
      const comma = result.indexOf(",");
      if (comma === -1) {
        reject(new Error("Invalid data URL"));
        return;
      }
      resolve(result.slice(comma + 1));
    };
    reader.onerror = () => reject(reader.error || new Error("Failed to read file"));
    reader.readAsDataURL(file);
  });

  const handlePaste = (e: ClipboardEvent) => {
    const items = e.clipboardData?.items;
    if (!items || items.length === 0) return;

    const imageFiles: File[] = [];
    for (let i = 0; i < items.length; i += 1) {
      const item = items[i];
      if (item.kind === "file" && item.type.startsWith("image/")) {
        const file = item.getAsFile();
        if (file) imageFiles.push(file);
      }
    }

    if (imageFiles.length === 0) return;
    e.preventDefault();

    void Promise.all(imageFiles.map(async (file) => {
      const data = await toBase64(file);
      return {
        id: crypto.randomUUID(),
        name: file.name || `pasted-image-${Date.now()}.png`,
        mediaType: file.type || "image/png",
        data,
      };
    })).then((newImages) => {
      setPastedImages((prev) => [...prev, ...newImages]);
      setShowImagesPanel(true);
    }).catch((err) => {
      console.error("Failed to process pasted image:", err);
    });
  };

  const handleRemovePastedImage = (id: string) => {
    setPastedImages((prev) => prev.filter((img) => img.id !== id));
  };

  const handleSubmit = (e: Event) => {
    e.preventDefault();
    const message = input().trim();
    if (!message || props.isRunning) return;

    // Join all selected paths with comma for Docker mounting
    const projectPath = selectedPaths().length > 0 ? selectedPaths().join(",") : undefined;

    const imagePaths = selectedImages();
    const imageData = pastedImages().map((img) => ({
      name: img.name,
      media_type: img.mediaType,
      data: img.data,
    }));
    const modelSupportsVision = isLikelyVisionModel(settings().model, settings().baseUrl);
    if ((imagePaths.length > 0 || imageData.length > 0) && !modelSupportsVision) {
      const proceed = confirm(
        `The selected model (${settings().model}) may not support image input. Continue anyway?`
      );
      if (!proceed) {
        return;
      }
    }

    if (isInConversation()) {
      // Continue existing conversation
      props.onContinueTask(
        message,
        projectPath,
        imagePaths.length > 0 ? imagePaths : undefined,
        imageData.length > 0 ? imageData : undefined
      );
    } else {
      // Create new task
      const firstLine = message.split("\n")[0];
      const title = firstLine.length > 50 ? firstLine.slice(0, 50) + "..." : firstLine;
      props.onNewTask(
        title,
        message,
        projectPath,
        imagePaths.length > 0 ? imagePaths : undefined,
        imageData.length > 0 ? imageData : undefined
      );
    }
    setInput("");
    setSelectedImages([]);
    setPastedImages([]);
    setShowImagesPanel(false);
  };

  return (
    <div class="agent-main">
      <Show
        when={isConfigured()}
        fallback={
          <div class="agent-setup">
            <h2>Welcome to Kuse Cowork</h2>
            <p>Configure your API key to start using the agent</p>
            <button onClick={toggleSettings}>Open Settings</button>
          </div>
        }
      >
        <div class="agent-content">
          {/* Output area */}
          <div class="agent-output">
            <Show
              when={props.activeTask || props.currentText || props.messages.length > 0}
              fallback={
                <div class="empty-state">
                  <h2>Agent Mode</h2>
                  <p>Describe a task and the agent will create a plan and execute it step by step.</p>
                  <div class="capabilities">
                    <div class="capability">
                      <span class="capability-icon">📁</span>
                      <span>Read, write, and edit files</span>
                    </div>
                    <div class="capability">
                      <span class="capability-icon">🔍</span>
                      <span>Search and explore codebases</span>
                    </div>
                    <div class="capability">
                      <span class="capability-icon">⚡</span>
                      <span>Run commands and scripts</span>
                    </div>
                    <div class="capability">
                      <span class="capability-icon">🐳</span>
                      <span>Execute in Docker containers</span>
                    </div>
                  </div>
                </div>
              }
            >
              {/* Show saved message history */}
              <For each={props.messages}>
                {(message) => (
                  <div class={`message ${message.role}`}>
                    <div class="message-label">
                      {message.role === "user" ? "You" : "Agent"}
                    </div>
                    <div class="message-content">{message.content}</div>
                  </div>
                )}
              </For>

              {/* Show current streaming text (when running a new task) */}
              <Show when={props.currentText && props.isRunning}>
                <div class="message assistant streaming">
                  <div class="message-label">Agent</div>
                  <div class="message-content">{props.currentText}</div>
                </div>
              </Show>
            </Show>
          </div>

          {/* Input area */}
          <div class="agent-input-area">
            {/* Selected paths panel */}
            <Show when={showPathsPanel() && selectedPaths().length > 0}>
              <div class="selected-paths">
                <div class="paths-header">
                  <span class="paths-label">Mounted Folders ({selectedPaths().length})</span>
                  <button
                    type="button"
                    class="paths-close"
                    onClick={() => setShowPathsPanel(false)}
                    title="Hide paths"
                  >
                    ×
                  </button>
                </div>
                <div class="paths-list">
                  <For each={selectedPaths()}>
                    {(path) => (
                      <div class="path-item">
                        <span class="path-icon">📁</span>
                        <span class="path-text" title={path}>
                          {path.split("/").pop() || path}
                        </span>
                        <button
                          type="button"
                          class="path-remove"
                          onClick={() => handleRemovePath(path)}
                          disabled={props.isRunning}
                          title={`Remove ${path}`}
                        >
                          ×
                        </button>
                      </div>
                    )}
                  </For>
                </div>
              </div>
            </Show>

            <Show when={showImagesPanel() && (selectedImages().length > 0 || pastedImages().length > 0)}>
              <div class="selected-paths image-attachments">
                <div class="paths-header">
                  <span class="paths-label">Attached Images ({selectedImages().length + pastedImages().length})</span>
                  <button
                    type="button"
                    class="paths-close"
                    onClick={() => setShowImagesPanel(false)}
                    title="Hide images"
                  >
                    ×
                  </button>
                </div>
                <div class="paths-list">
                  <For each={selectedImages()}>
                    {(path) => (
                      <div class="path-item">
                        <span class="path-icon">🖼️</span>
                        <span class="path-text" title={path}>
                          {path.split("\\").pop() || path}
                        </span>
                        <button
                          type="button"
                          class="path-remove"
                          onClick={() => handleRemoveImage(path)}
                          disabled={props.isRunning}
                          title={`Remove ${path}`}
                        >
                          ×
                        </button>
                      </div>
                    )}
                  </For>
                  <For each={pastedImages()}>
                    {(img) => (
                      <div class="path-item">
                        <span class="path-icon">🖼️</span>
                        <span class="path-text" title={img.name}>
                          {img.name} (pasted)
                        </span>
                        <button
                          type="button"
                          class="path-remove"
                          onClick={() => handleRemovePastedImage(img.id)}
                          disabled={props.isRunning}
                          title={`Remove ${img.name}`}
                        >
                          ×
                        </button>
                      </div>
                    )}
                  </For>
                </div>
              </div>
            </Show>

            <Show when={(selectedImages().length > 0 || pastedImages().length > 0) && !isLikelyVisionModel(settings().model, settings().baseUrl)}>
              <div class="image-warning">
                Attached images may be ignored by current model:
                <strong> {settings().model}</strong>
              </div>
            </Show>

            <form class="agent-form" onSubmit={handleSubmit}>
              <div class="input-row">
                <textarea
                  value={input()}
                  onInput={(e) => setInput(e.currentTarget.value)}
                  onPaste={handlePaste}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" && !e.shiftKey) {
                      e.preventDefault();
                      handleSubmit(e);
                    }
                  }}
                  placeholder={isInConversation()
                    ? "Continue the conversation..."
                    : "Describe a task... (e.g., 'Find and fix the authentication bug in auth.ts')"
                  }
                  disabled={props.isRunning}
                  rows={3}
                />
                <div class="input-actions">
                  <button
                    type="button"
                    class={`path-toggle ${(selectedImages().length > 0 || pastedImages().length > 0) ? "active" : ""}`}
                    onClick={handleAddImages}
                    disabled={props.isRunning}
                    title="Attach images (or paste into text box)"
                  >
                    🖼️
                    <Show when={(selectedImages().length + pastedImages().length) > 0}>
                      <span class="path-count">{selectedImages().length + pastedImages().length}</span>
                    </Show>
                  </button>
                  <button
                    type="button"
                    class={`path-toggle ${selectedPaths().length > 0 ? "active" : ""}`}
                    onClick={handleAddFolders}
                    disabled={props.isRunning}
                    title="Add folders to mount"
                  >
                    📁
                    <Show when={selectedPaths().length > 0}>
                      <span class="path-count">{selectedPaths().length}</span>
                    </Show>
                  </button>
                  <Show when={isInConversation()}>
                    <button
                      type="button"
                      class="new-chat-btn ghost"
                      onClick={props.onNewConversation}
                      disabled={props.isRunning}
                      title="Start new conversation"
                    >
                      +
                    </button>
                  </Show>
                  <button type="submit" class="submit-btn" disabled={props.isRunning || !input().trim()}>
                    {props.isRunning ? "Running..." : isInConversation() ? "Send" : "Start Task"}
                  </button>
                </div>
              </div>
            </form>
          </div>
        </div>
      </Show>
    </div>
  );
};

export default AgentMain;
