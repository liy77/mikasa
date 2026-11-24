import { spawn } from 'child_process';
import * as path from 'path';
import * as vscode from 'vscode';

/**
 * Debug adapter for Mikasa.
 *
 * It runs the file through Astra (instant) or `mikasa` (transpile to Rust),
 * streams the output into the debug console, and honours `stopOnEntry` by
 * pausing on line 1 before the run so breakpoints can be set. The runtime
 * itself has no breakpoint protocol yet, so a stop is the entry stop — the
 * adapter says so rather than pretending it stepped.
 */
export function registerDebugger(context: vscode.ExtensionContext): void {
  const factory: vscode.DebugAdapterDescriptorFactory = {
    createDebugAdapterDescriptor() {
      return new vscode.DebugAdapterInlineImplementation(new MikasaDebugSession());
    },
  };
  context.subscriptions.push(
    vscode.debug.registerDebugAdapterDescriptorFactory('mikasa', factory)
  );

  context.subscriptions.push(
    vscode.commands.registerCommand('mikasa.runFile', () => launch(false)),
    vscode.commands.registerCommand('mikasa.debugFile', () => launch(true))
  );
}

async function launch(stopOnEntry: boolean): Promise<void> {
  const editor = vscode.window.activeTextEditor;
  if (!editor || editor.document.languageId !== 'copper') {
    vscode.window.showWarningMessage('Mikasa: open a .mika file first.');
    return;
  }
  await vscode.debug.startDebugging(undefined, {
    name: 'Mikasa',
    type: 'mikasa',
    request: 'launch',
    program: editor.document.uri.fsPath,
    stopOnEntry,
    runtime: vscode.workspace.getConfiguration('mikasa').get('runtime', 'astra'),
  });
}

class MikasaDebugSession implements vscode.DebugAdapter {
  private sendMessage = new vscode.EventEmitter<vscode.DebugProtocolMessage>();
  private seq = 1;

  readonly onDidSendMessage = this.sendMessage.event;

  handleMessage(message: vscode.DebugProtocolMessage): void {
    const msg = message as { command?: string; seq: number; arguments?: LaunchArgs };
    const reply = (body: unknown, success = true) =>
      this.emit({
        seq: this.seq++,
        type: 'response',
        request_seq: msg.seq,
        command: msg.command,
        success,
        body,
      });

    switch (msg.command) {
      case 'initialize':
        reply({ supportsConfigurationDoneRequest: true });
        this.event('initialized', {});
        break;
      case 'launch':
        reply({});
        void this.run(msg.arguments ?? {});
        break;
      case 'configurationDone':
      case 'threads':
        reply({ threads: [{ id: 1, name: 'main' }] });
        break;
      case 'disconnect':
        reply({});
        this.event('terminated', {});
        break;
      default:
        reply({});
    }
  }

  private async run(args: LaunchArgs): Promise<void> {
    const program = args.program ?? '';
    if (args.stopOnEntry) {
      this.event('stopped', {
        reason: 'entry',
        threadId: 1,
        description: 'entry — the runtime has no per-line breakpoints yet',
        allThreadsStopped: true,
      });
    }

    const config = vscode.workspace.getConfiguration('mikasa');
    const runtime = args.runtime ?? config.get('runtime', 'astra');
    const command =
      runtime === 'mikasa'
        ? config.get('mikasaPath', 'mikasa')
        : config.get('astraPath', 'astra');
    const runArgs = runtime === 'mikasa' ? ['run', program] : ['run', program];

    this.event('output', {
      category: 'console',
      output: `${command} ${runArgs.join(' ')}\n`,
    });

    const child = spawn(command, runArgs, { cwd: path.dirname(program) });

    child.stdout?.on('data', (buf: Buffer) =>
      this.event('output', { category: 'stdout', output: buf.toString() })
    );
    child.stderr?.on('data', (buf: Buffer) =>
      this.event('output', { category: 'stderr', output: buf.toString() })
    );
    child.on('close', (code: number | null) => {
      this.event('output', {
        category: 'console',
        output: `\nexited with code ${code ?? 0}\n`,
      });
      this.event('terminated', {});
    });
    child.on('error', (err: Error) => {
      this.event('output', {
        category: 'stderr',
        output: `could not start ${command}: ${err.message}\n`,
      });
      this.event('terminated', {});
    });
  }

  private event(event: string, body: unknown): void {
    this.emit({ seq: this.seq++, type: 'event', event, body });
  }

  private emit(message: object): void {
    this.sendMessage.fire(message as vscode.DebugProtocolMessage);
  }

  dispose(): void {
    this.sendMessage.dispose();
  }
}

interface LaunchArgs {
  program?: string;
  stopOnEntry?: boolean;
  runtime?: string;
}
