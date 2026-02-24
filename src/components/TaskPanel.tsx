import { Component, For, Show } from "solid-js";
import { Task } from "../lib/tauri-api";
import "./TaskPanel.css";

interface TaskPanelProps {
  task: Task | null;
  isRunning: boolean;
  toolExecutions: ToolExecution[];
}

interface ToolExecution {
  id: number;
  tool: string;
  status: "running" | "completed" | "error";
  input?: string;
  result?: string;
  success?: boolean;
}

const TaskPanel: Component<TaskPanelProps> = (props) => {
  const getStepIcon = (status: string) => {
    switch (status) {
      case "completed":
        return "OK";
      case "running":
        return "..";
      case "failed":
        return "ERR";
      default:
        return "--";
    }
  };

  const getStatusColor = (status: string) => {
    switch (status) {
      case "completed":
        return "var(--success)";
      case "running":
        return "var(--primary)";
      case "failed":
        return "var(--error)";
      default:
        return "var(--muted-foreground)";
    }
  };

  return (
    <div class="task-panel">
      <Show
        when={props.task}
        fallback={
          <div class="task-panel-empty">
            <p>No active task</p>
            <p class="hint">Start a task from the main panel</p>
          </div>
        }
      >
        {(task) => (
          <>
            <div class="task-header">
              <div class="task-title">{task().title}</div>
              <div class={`task-status ${task().status}`}>
                {task().status === "planning" && "Planning..."}
                {task().status === "running" && "Running"}
                {task().status === "completed" && "Completed"}
                {task().status === "failed" && "Failed"}
              </div>
            </div>

            <div class="task-description">{task().description}</div>

            <Show when={task().plan && task().plan!.length > 0}>
              <div class="plan-section">
                <div class="plan-header">Plan</div>
                <div class="plan-steps">
                  <For each={task().plan}>
                    {(step) => (
                      <div class={`plan-step ${step.status}`}>
                        <span class="step-icon" style={{ color: getStatusColor(step.status) }}>
                          {getStepIcon(step.status)}
                        </span>
                        <span class="step-number">{step.step}.</span>
                        <span class="step-description">{step.description}</span>
                      </div>
                    )}
                  </For>
                </div>
              </div>
            </Show>

            <Show when={props.toolExecutions.length > 0}>
              <div class="tools-section">
                <div class="tools-header">Tools</div>
                <div class="tool-list">
                  <For each={props.toolExecutions}>
                    {(tool) => (
                      <div class={`tool-item ${tool.status}`}>
                        <div class="tool-item-header">
                          <span class="tool-name">{tool.tool}</span>
                          <span class="tool-status-icon">
                            {tool.status === "running" && "..."}
                            {tool.status === "completed" && "OK"}
                            {tool.status === "error" && "ERR"}
                          </span>
                        </div>
                        <Show when={tool.input}>
                          <details class="tool-details">
                            <summary>Input</summary>
                            <pre>{tool.input}</pre>
                          </details>
                        </Show>
                        <Show when={tool.result}>
                          <details class="tool-details" open={tool.status === "error"}>
                            <summary>{tool.status === "error" ? "Error Output" : "Output"}</summary>
                            <pre>{tool.result}</pre>
                          </details>
                        </Show>
                      </div>
                    )}
                  </For>
                </div>
              </div>
            </Show>

            <Show when={props.isRunning}>
              <div class="running-indicator">
                <span class="pulse"></span>
                <span>Working...</span>
              </div>
            </Show>
          </>
        )}
      </Show>
    </div>
  );
};

export default TaskPanel;
