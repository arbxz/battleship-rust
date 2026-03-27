import * as vscode from "vscode";
import * as path from "path";
import * as fs from "fs";

let gameTerminal: vscode.Terminal | undefined;

function getBinaryPath(context: vscode.ExtensionContext): string {
  // Look for bundled binary first, then fall back to PATH
  const ext = process.platform === "win32" ? ".exe" : "";
  const bundled = path.join(context.extensionPath, "bin", `rust-vader${ext}`);
  if (fs.existsSync(bundled)) {
    return `"${bundled}"`;
  }
  return "rust-vader";
}

export function activate(context: vscode.ExtensionContext) {
  const binaryPath = getBinaryPath(context);

  const disposable = vscode.commands.registerCommand("rustVader.play", () => {
    // Reuse terminal if it's still open
    if (gameTerminal && !gameTerminal.exitStatus) {
      gameTerminal.show();
      gameTerminal.sendText(binaryPath);
      return;
    }

    gameTerminal = vscode.window.createTerminal({
      name: "Rust Vader",
      hideFromUser: false,
    });

    gameTerminal.show();
    gameTerminal.sendText(binaryPath);
  });

  // Clean up reference when a terminal closes
  const onClose = vscode.window.onDidCloseTerminal((t) => {
    if (t === gameTerminal) {
      gameTerminal = undefined;
    }
  });

  context.subscriptions.push(disposable, onClose);
}

export function deactivate() {
  gameTerminal = undefined;
}
