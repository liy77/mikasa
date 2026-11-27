import * as fs from 'fs';
import * as path from 'path';
import * as vscode from 'vscode';
import { registerDebugger } from './debug';
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  TransportKind,
} from 'vscode-languageclient/node';

let client: LanguageClient | undefined;

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  const config = vscode.workspace.getConfiguration('mikasa');
  const rawPath = config.get<string>('serverPath', 'mikasa-lsp');
  let serverPath = resolvePath(rawPath);

  // When the user left the default (bare `mikasa-lsp`), try to locate a built
  // binary under a `target/{release,debug}` near the workspace or extension —
  // so diagnostics work in the dev tree without putting mikasa-lsp on PATH.
  if (rawPath === 'mikasa-lsp') {
    const discovered = locateServer(context);
    if (discovered) {
      serverPath = discovered;
    }
  }

  // Surface the resolved path so users can debug "couldn't create connection
  // to server" errors. Goes to the extension host log.
  console.log(`[mikasa] LSP serverPath (raw)=${rawPath} resolved=${serverPath}`);

  if (path.isAbsolute(serverPath) && !fs.existsSync(serverPath)) {
    vscode.window.showErrorMessage(
      `mikasa-lsp not found at "${serverPath}". Set "mikasa.serverPath" or build the binary (\`cargo build --release -p mikasa-lsp\`).`
    );
    return;
  }

  const serverOptions: ServerOptions = {
    run: { command: serverPath, transport: TransportKind.stdio },
    debug: { command: serverPath, transport: TransportKind.stdio },
  };

  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ scheme: 'file', language: 'mikasa' }],
    synchronize: {
      fileEvents: vscode.workspace.createFileSystemWatcher('**/*.{mika,crs}'),
    },
  };

  client = new LanguageClient(
    'mikasa',
    'Mikasa Language Server',
    serverOptions,
    clientOptions
  );

  try {
    await client.start();
  } catch (err) {
    vscode.window.showErrorMessage(
      `Could not start mikasa-lsp at "${serverPath}". Set "mikasa.serverPath" or install the binary on PATH. (${err})`
    );
  }

  registerDebugger(context);

  context.subscriptions.push(
    vscode.commands.registerCommand('mikasa.restartLanguageServer', async () => {
      if (!client) {
        vscode.window.showWarningMessage('Mikasa: no language server is running.');
        return;
      }
      await vscode.window.withProgress(
        {
          location: vscode.ProgressLocation.Notification,
          title: 'Restarting Mikasa Language Server…',
          cancellable: false,
        },
        async () => {
          await client!.stop();
          await client!.start();
        }
      );
    })
  );
}

/**
 * Expand `${workspaceFolder}` and `${workspaceFolder:NAME}` placeholders.
 * VSCode resolves these automatically inside `launch.json`/`tasks.json` but
 * not inside arbitrary extension settings — the extension has to do it.
 */
function resolvePath(input: string): string {
  const folders = vscode.workspace.workspaceFolders ?? [];
  const primary = folders[0]?.uri.fsPath;

  let out = input.replace(/\$\{workspaceFolder(?::([^}]+))?\}/g, (_match, name) => {
    if (name) {
      const found = folders.find((f) => f.name === name);
      return found ? found.uri.fsPath : '';
    }
    return primary ?? '';
  });

  // Tilde-expansion for ~/ paths.
  if (out.startsWith('~/') || out.startsWith('~\\')) {
    out = path.join(require('os').homedir(), out.slice(2));
  }

  // Normalize separators on Windows so a posix-style path still resolves.
  if (process.platform === 'win32' && out.includes('/')) {
    out = path.normalize(out);
  }

  return out;
}

/**
 * Locate a built `mikasa-lsp` binary under a `target/{release,debug}` directory
 * by walking up from the workspace folders and the extension itself. Returns the
 * path, or undefined to fall back to a PATH lookup of the bare name.
 */
function locateServer(context: vscode.ExtensionContext): string | undefined {
  const exe = process.platform === 'win32' ? 'mikasa-lsp.exe' : 'mikasa-lsp';
  const roots: string[] = [];
  for (const f of vscode.workspace.workspaceFolders ?? []) {
    roots.push(f.uri.fsPath);
  }
  roots.push(context.extensionPath);

  for (const root of roots) {
    let dir = root;
    for (let i = 0; i < 6; i++) {
      for (const profile of ['release', 'debug']) {
        const cand = path.join(dir, 'target', profile, exe);
        if (fs.existsSync(cand)) {
          return cand;
        }
      }
      const parent = path.dirname(dir);
      if (parent === dir) break;
      dir = parent;
    }
  }
  return undefined;
}

export async function deactivate(): Promise<void> {
  if (client) {
    await client.stop();
    client = undefined;
  }
}
