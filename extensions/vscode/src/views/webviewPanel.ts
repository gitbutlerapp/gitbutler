import * as vscode from 'vscode';
import { WorkspaceService, type WorkspaceState } from '../services/workspace';
import { ButApiClient, pathToBytes } from '../services/api';
import { generateMessage } from '../commands';
import type { FileChange, Stack, Segment } from '../types/gitbutler';

/**
 * WebviewViewProvider for the GitButler panel in the Source Control sidebar.
 * Renders branches with commit inputs + file trees + unassigned changes.
 */
export class GitButlerWebviewProvider implements vscode.WebviewViewProvider {
  private view?: vscode.WebviewView;
  private state: WorkspaceState | null = null;


  constructor(
    private readonly workspace: WorkspaceService,
    private readonly api: ButApiClient,
    private readonly extensionUri: vscode.Uri
  ) {
    workspace.onDidChangeState((state) => {
      this.state = state;
      this.updateWebview();
    });
  }

  resolveWebviewView(
    webviewView: vscode.WebviewView,
    _context: vscode.WebviewViewResolveContext,
    _token: vscode.CancellationToken
  ): void {
    this.view = webviewView;

    webviewView.webview.options = {
      enableScripts: true,
      localResourceRoots: [this.extensionUri],
    };

    // Handle messages from the webview
    webviewView.webview.onDidReceiveMessage(async (message) => {
      switch (message.type) {
        case 'commit': {
          await this.handleCommit(message.stackId, message.branchName, message.message);
          break;
        }
        case 'stage': {
          await this.handleStage(message.filePath, message.stackId);
          break;
        }
        case 'unstage': {
          await this.handleUnstage(message.filePath);
          break;
        }
        case 'moveToBranch': {
          await this.handleMove(message.filePath, message.targetStackId);
          break;
        }
        case 'discard': {
          await this.handleDiscard(message.filePath);
          break;
        }
        case 'openFile': {
          const uri = vscode.Uri.file(
            require('path').join(
              vscode.workspace.workspaceFolders?.[0]?.uri.fsPath || '',
              message.filePath
            )
          );
          vscode.window.showTextDocument(uri);
          break;
        }
        case 'createBranch': {
          vscode.commands.executeCommand('gitbutler.createBranch');
          break;
        }
        case 'refresh': {
          await this.workspace.refresh();
          break;
        }
        case 'stageFiles': {
          await this.handleStageFiles(message.filePaths, message.stackId);
          break;
        }
        case 'unstageFiles': {
          await this.handleUnstageFiles(message.filePaths);
          break;
        }
        case 'moveFiles': {
          await this.handleMoveFiles(message.filePaths, message.targetStackId);
          break;
        }
        case 'generateCommitMessage': {
          await this.handleGenerateCommitMessage(message.stackId, message.branchName);
          break;
        }
        case 'pushBranch': {
          await this.handlePushBranch(message.stackId, message.branchName);
          break;
        }
        case 'deleteBranch': {
          await this.handleDeleteBranch(message.stackId, message.branchName);
          break;
        }
      }
    });

    this.updateWebview();
  }

  private updateWebview(): void {
    if (!this.view) return;
    this.view.webview.html = this.getHtml();
  }

  private async handleCommit(stackId: string, branchName: string, message: string): Promise<void> {
    if (!message.trim()) {
      vscode.window.showWarningMessage('GitButler: Commit message is empty');
      return;
    }
    const changesForStack = (this.state?.stagedChanges || []).filter((c) => c.stackId === stackId);
    if (changesForStack.length === 0) {
      vscode.window.showWarningMessage('GitButler: No changes assigned to this branch');
      return;
    }
    try {
      const worktreeChanges = changesForStack.map((c) => ({
        previousPathBytes: c.treeChange.status.type === 'Rename' ? c.treeChange.status.subject.previousPathBytes : null,
        pathBytes: c.treeChange.pathBytes,
        hunkHeaders: [],
      }));
      await this.api.createCommit({ stackId, message, stackBranchName: branchName, worktreeChanges });
      await this.workspace.refresh();
      vscode.window.showInformationMessage(`GitButler: Committed "${message.substring(0, 50)}"`);
    } catch (err: any) {
      vscode.window.showErrorMessage(`Commit failed: ${err?.detail || err?.message || err}`);
    }
  }

  private async handleStage(filePath: string, stackId: string): Promise<void> {
    try {
      await this.api.assignHunk([{
        hunkHeader: null,
        pathBytes: pathToBytes(filePath),
        stackId,
      }]);
      await this.workspace.refresh();
    } catch (err: any) {
      vscode.window.showErrorMessage(`Stage failed: ${err?.detail || err?.message || err}`);
    }
  }

  private async handleUnstage(filePath: string): Promise<void> {
    try {
      await this.api.assignHunk([{
        hunkHeader: null,
        pathBytes: pathToBytes(filePath),
        stackId: null,
      }]);
      await this.workspace.refresh();
    } catch (err: any) {
      vscode.window.showErrorMessage(`Unstage failed: ${err?.detail || err?.message || err}`);
    }
  }

  private async handleMove(filePath: string, targetStackId: string): Promise<void> {
    try {
      const rejections = await this.api.assignHunk([{
        hunkHeader: null,
        pathBytes: pathToBytes(filePath),
        stackId: targetStackId,
      }]) as any[];
      await this.workspace.refresh();
      if (rejections && rejections.length > 0) {
        vscode.window.showWarningMessage(`GitButler: Some hunks could not be moved (locked by existing commits)`);
      }
    } catch (err: any) {
      vscode.window.showErrorMessage(`Move failed: ${err?.detail || err?.message || err}`);
    }
  }

  private async handleDiscard(filePath: string): Promise<void> {
    try {
      await this.api.discardChanges([{
        previousPathBytes: null,
        pathBytes: pathToBytes(filePath),
        hunkHeaders: [],
      }]);
      await this.workspace.refresh();
    } catch (err: any) {
      vscode.window.showErrorMessage(`Discard failed: ${err?.detail || err?.message || err}`);
    }
  }

  private async handleStageFiles(filePaths: string[], stackId: string): Promise<void> {
    try {
      await this.api.assignHunk(filePaths.map((fp) => ({
        hunkHeader: null,
        pathBytes: pathToBytes(fp),
        stackId,
      })));
      await this.workspace.refresh();
    } catch (err: any) {
      vscode.window.showErrorMessage(`Stage failed: ${err?.detail || err?.message || err}`);
    }
  }

  private async handleUnstageFiles(filePaths: string[]): Promise<void> {
    try {
      await this.api.assignHunk(filePaths.map((fp) => ({
        hunkHeader: null,
        pathBytes: pathToBytes(fp),
        stackId: null,
      })));
      await this.workspace.refresh();
    } catch (err: any) {
      vscode.window.showErrorMessage(`Unstage failed: ${err?.detail || err?.message || err}`);
    }
  }

  private async handleMoveFiles(filePaths: string[], targetStackId: string): Promise<void> {
    try {
      const rejections = await this.api.assignHunk(filePaths.map((fp) => ({
        hunkHeader: null,
        pathBytes: pathToBytes(fp),
        stackId: targetStackId,
      }))) as any[];
      await this.workspace.refresh();
      if (rejections && rejections.length > 0) {
        vscode.window.showWarningMessage(`GitButler: ${rejections.length} hunk(s) could not be moved (locked by existing commits)`);
      }
    } catch (err: any) {
      vscode.window.showErrorMessage(`Move failed: ${err?.detail || err?.message || err}`);
    }
  }

  private async handleGenerateCommitMessage(stackId: string, branchName: string): Promise<void> {
    const changes = (this.state?.stagedChanges || []).filter((c) => c.stackId === stackId);
    if (changes.length === 0) {
      vscode.window.showWarningMessage('GitButler: No changes assigned to this branch');
      return;
    }
    try {
      const message = await vscode.window.withProgress(
        { location: vscode.ProgressLocation.Notification, title: 'Generating commit message...' },
        () => generateMessage(changes, branchName)
      );
      if (message && this.view) {
        this.view.webview.postMessage({ type: 'setCommitMessage', stackId, message });
      }
    } catch (err: any) {
      vscode.window.showErrorMessage(`AI generate failed: ${err?.detail || err?.message || err}`);
    }
  }

  private async handlePushBranch(stackId: string, branchName: string): Promise<void> {
    try {
      await vscode.window.withProgress(
        { location: vscode.ProgressLocation.Notification, title: `Pushing ${branchName}...` },
        () => this.api.pushStack({ stackId, branch: branchName })
      );
      await this.workspace.refresh();
      vscode.window.showInformationMessage(`GitButler: Pushed "${branchName}"`);
    } catch (err: any) {
      vscode.window.showErrorMessage(`Push failed: ${err?.detail || err?.message || err}`);
    }
  }

  private async handleDeleteBranch(stackId: string, branchName: string): Promise<void> {
    const confirm = await vscode.window.showWarningMessage(
      `Delete branch "${branchName}"?`,
      { modal: true },
      'Delete'
    );
    if (confirm !== 'Delete') return;
    try {
      await this.api.removeBranch(stackId, branchName);
      await this.workspace.refresh();
      vscode.window.showInformationMessage(`GitButler: Deleted branch "${branchName}"`);
    } catch (err: any) {
      vscode.window.showErrorMessage(`Delete failed: ${err?.detail || err?.message || err}`);
    }
  }

  // ---------------------------------------------------------------------------
  // HTML generation
  // ---------------------------------------------------------------------------

  private getHtml(): string {
    const stacks = this.state?.stacks || [];
    const changes = this.state?.changes || [];
    const stagedChanges = this.state?.stagedChanges || [];
    const isReady = this.state?.isReady ?? false;

    // Build branch data for drag-drop targets
    const branchData = stacks.flatMap((stack) =>
      stack.segments.filter((s) => s.refName).map((seg) => ({
        stackId: stack.id!,
        name: seg.refName!.displayName,
      }))
    );

    const codiconCssUri = this.view!.webview.asWebviewUri(
      vscode.Uri.joinPath(this.extensionUri, 'dist', 'codicon.css')
    );
    const codiconFontUri = this.view!.webview.asWebviewUri(
      vscode.Uri.joinPath(this.extensionUri, 'dist', 'codicon.ttf')
    );

    return /* html */ `<!DOCTYPE html>
<html>
<head>
<meta charset="UTF-8">
<link rel="stylesheet" href="${codiconCssUri}">
<style>
  @font-face {
    font-family: "codicon";
    font-display: block;
    src: url("${codiconFontUri}") format("truetype");
  }
</style>
<style>
  :root {
    --vscode-font-family: var(--vscode-editor-font-family, sans-serif);
  }
  * { box-sizing: border-box; margin: 0; padding: 0; }
  body {
    font-family: var(--vscode-font-family);
    font-size: var(--vscode-font-size, 13px);
    color: var(--vscode-foreground);
    background: transparent;
    padding: 0;
    overflow-x: hidden;
  }

  .section {
    margin-bottom: 4px;
  }

  .section-header {
    display: flex;
    align-items: center;
    padding: 4px 8px;
    background: var(--vscode-sideBarSectionHeader-background);
    border-bottom: 1px solid var(--vscode-sideBarSectionHeader-border, transparent);
    cursor: pointer;
    user-select: none;
    font-weight: 600;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    gap: 4px;
  }
  .section-header:hover {
    background: var(--vscode-list-hoverBackground);
  }
  .section-header .chevron {
    font-size: 14px;
    width: 16px;
    text-align: center;
  }
  .section-header .badge {
    margin-left: auto;
    background: var(--vscode-badge-background);
    color: var(--vscode-badge-foreground);
    border-radius: 10px;
    padding: 1px 6px;
    font-size: 11px;
    font-weight: normal;
  }
  .section-header .actions {
    margin-left: auto;
    display: flex;
    gap: 2px;
  }
  .section-header .actions button {
    background: none;
    border: none;
    color: var(--vscode-foreground);
    cursor: pointer;
    padding: 2px 4px;
    border-radius: 3px;
    font-size: 14px;
    opacity: 0.7;
  }
  .section-header .actions button:hover {
    opacity: 1;
    background: var(--vscode-toolbar-hoverBackground);
  }

  .section-body {
    overflow: hidden;
  }
  .section-body.collapsed {
    display: none;
  }

  /* Commit input */
  .commit-area {
    padding: 6px 8px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .commit-input-wrap {
    position: relative;
    display: flex;
    align-items: stretch;
  }
  .commit-input {
    flex: 1;
    background: var(--vscode-input-background);
    color: var(--vscode-input-foreground);
    border: 1px solid var(--vscode-input-border, transparent);
    border-radius: 3px;
    padding: 4px 28px 4px 6px;
    font-family: var(--vscode-font-family);
    font-size: var(--vscode-font-size, 13px);
    resize: none;
    min-height: 26px;
    max-height: 80px;
    outline: none;
  }
  .commit-input:focus {
    border-color: var(--vscode-focusBorder);
  }
  .commit-input::placeholder {
    color: var(--vscode-input-placeholderForeground);
  }
  .ai-btn {
    position: absolute;
    right: 2px;
    top: 2px;
    bottom: 2px;
    background: none;
    border: none;
    cursor: pointer;
    padding: 0 5px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--vscode-foreground);
    opacity: 0.5;
    border-radius: 2px;
  }
  .ai-btn:hover {
    opacity: 1;
    background: var(--vscode-toolbar-hoverBackground);
  }

  .commit-btn {
    background: var(--vscode-button-background);
    color: var(--vscode-button-foreground);
    border: none;
    border-radius: 3px;
    padding: 4px 10px;
    cursor: pointer;
    font-size: 12px;
    white-space: nowrap;
    height: 26px;
    width: 100%;
    text-align: center;
  }
  .commit-btn:hover {
    background: var(--vscode-button-hoverBackground);
  }
  .commit-btn:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .root-drag {
    border-bottom: 1px solid var(--vscode-panel-border, transparent);
    padding: 3px 8px 3px 12px;
    opacity: 0.85;
    font-size: 12px;
  }
  .root-drag:hover {
    opacity: 1;
  }

  /* File tree */
  .file-tree {
    list-style: none;
    padding: 0;
  }
  .file-item {
    display: flex;
    align-items: center;
    padding: 2px 8px 2px 20px;
    cursor: pointer;
    gap: 4px;
    user-select: none;
  }
  .file-item:hover {
    background: var(--vscode-list-hoverBackground);
  }
  .file-item.dragging {
    opacity: 0.4;
  }
  .file-item .icon {
    width: 16px;
    text-align: center;
    flex-shrink: 0;
  }
  .file-item .name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .file-item .dir {
    color: var(--vscode-descriptionForeground);
    font-size: 0.9em;
    margin-left: 4px;
  }
  .file-item .file-actions {
    display: none;
    gap: 2px;
  }
  .file-item:hover .file-actions {
    display: flex;
  }
  .file-item .file-actions button {
    background: none;
    border: none;
    color: var(--vscode-foreground);
    cursor: pointer;
    padding: 1px 3px;
    border-radius: 3px;
    font-size: 13px;
    opacity: 0.7;
  }
  .file-item .file-actions button:hover {
    opacity: 1;
    background: var(--vscode-toolbar-hoverBackground);
  }

  .folder-item {
    display: flex;
    align-items: center;
    padding: 2px 8px 2px 12px;
    cursor: pointer;
    gap: 4px;
    user-select: none;
    font-weight: 500;
  }
  .folder-item:hover {
    background: var(--vscode-list-hoverBackground);
  }
  .folder-item .file-actions {
    display: none;
    gap: 2px;
  }
  .folder-item:hover .file-actions {
    display: flex;
  }
  .folder-item .file-actions button {
    background: none;
    border: none;
    color: var(--vscode-foreground);
    cursor: pointer;
    padding: 1px 3px;
    border-radius: 3px;
    font-size: 13px;
    opacity: 0.7;
  }
  .folder-item .file-actions button:hover {
    opacity: 1;
    background: var(--vscode-toolbar-hoverBackground);
  }
  .folder-children {
    padding-left: 8px;
  }
  .folder-children.collapsed {
    display: none;
  }

  /* Push status */
  .push-status {
    font-size: 11px;
    margin-left: 4px;
    opacity: 0.7;
  }

  .status-added { color: var(--vscode-gitDecoration-addedResourceForeground, #73c991); }
  .status-modified { color: var(--vscode-gitDecoration-modifiedResourceForeground, #e2c08d); }
  .status-deleted { color: var(--vscode-gitDecoration-deletedResourceForeground, #c74e39); }
  .status-renamed { color: var(--vscode-gitDecoration-renamedResourceForeground, #73c991); }
  .status-untracked { color: var(--vscode-gitDecoration-untrackedResourceForeground, #73c991); }

  .drop-target {
    outline: 2px dashed var(--vscode-focusBorder);
    outline-offset: -2px;
    border-radius: 3px;
  }

  .empty-msg {
    padding: 8px 20px;
    color: var(--vscode-descriptionForeground);
    font-style: italic;
    font-size: 12px;
  }


</style>
</head>
<body>
  ${!isReady ? '<div class="empty-msg">GitButler is initializing...</div>' : this.renderBranches(stacks, stagedChanges) + this.renderUnassigned(changes, branchData)}

  <script>
    const vscode = acquireVsCodeApi();
    function send(msg) { vscode.postMessage(msg); }

    // Toggle chevron helper
    function toggleChevron(chevron) {
      if (!chevron) return;
      chevron.classList.toggle('codicon-chevron-down');
      chevron.classList.toggle('codicon-chevron-right');
    }

    // Toggle sections
    document.querySelectorAll('.section-header').forEach(h => {
      h.addEventListener('click', (e) => {
        if (e.target.closest('.actions') || e.target.closest('button')) return;
        const body = h.nextElementSibling;
        if (body) body.classList.toggle('collapsed');
        toggleChevron(h.querySelector('.chevron'));
      });
    });

    // Toggle folders
    document.querySelectorAll('.folder-item').forEach(f => {
      f.addEventListener('click', (e) => {
        if (e.target.closest('.file-actions') || e.target.closest('button')) return;
        const children = f.nextElementSibling;
        if (children && children.classList.contains('folder-children')) {
          children.classList.toggle('collapsed');
        }
        toggleChevron(f.querySelector('.chevron'));
      });
    });

    // Commit on Ctrl+Enter
    document.querySelectorAll('.commit-input').forEach(input => {
      input.addEventListener('keydown', (e) => {
        if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
          e.preventDefault();
          const stackId = input.dataset.stackId;
          const branchName = input.dataset.branchName;
          const msg = input.value.trim();
          if (msg) {
            send({ type: 'commit', stackId, branchName, message: msg });
            input.value = '';
          }
        }
      });
      // Auto-resize
      input.addEventListener('input', () => {
        input.style.height = 'auto';
        input.style.height = Math.min(input.scrollHeight, 80) + 'px';
      });
    });

    // Drag and drop — supports single files (data-path) and multi-file (data-paths)
    let dragData = null;

    document.querySelectorAll('[draggable="true"]').forEach(item => {
      item.addEventListener('dragstart', (e) => {
        e.stopPropagation();
        const paths = item.dataset.paths ? JSON.parse(item.dataset.paths) : null;
        const singlePath = item.dataset.path || null;
        dragData = {
          paths: paths || (singlePath ? [singlePath] : []),
          stackId: item.dataset.stackId || null
        };
        item.classList.add('dragging');
        e.dataTransfer.effectAllowed = 'move';
      });
      item.addEventListener('dragend', () => {
        item.classList.remove('dragging');
        document.querySelectorAll('.drop-target').forEach(el => el.classList.remove('drop-target'));
        dragData = null;
      });
    });

    // Drop targets — use closest section to resolve the right target
    let currentDropTarget = null;

    function findDropSection(el) {
      if (!el || !el.closest) return null;
      return el.closest('[data-drop-stack-id]');
    }

    document.addEventListener('dragover', (e) => {
      if (!dragData) return;
      e.preventDefault();
      e.dataTransfer.dropEffect = 'move';
      const section = findDropSection(e.target);
      if (section !== currentDropTarget) {
        if (currentDropTarget) currentDropTarget.classList.remove('drop-target');
        currentDropTarget = section;
        if (currentDropTarget) currentDropTarget.classList.add('drop-target');
      }
    });

    document.addEventListener('dragleave', (e) => {
      if (!e.relatedTarget || !document.contains(e.relatedTarget)) {
        if (currentDropTarget) currentDropTarget.classList.remove('drop-target');
        currentDropTarget = null;
      }
    });

    document.addEventListener('drop', (e) => {
      e.preventDefault();
      if (currentDropTarget) currentDropTarget.classList.remove('drop-target');
      if (!dragData || dragData.paths.length === 0) { currentDropTarget = null; return; }

      const section = findDropSection(e.target);
      if (!section) {
        console.log('[GitButler DnD] drop: no section found for', e.target);
        currentDropTarget = null; dragData = null; return;
      }

      const targetStackId = section.dataset.dropStackId;
      const sourceStackId = dragData.stackId || '';
      console.log('[GitButler DnD] drop:', dragData.paths.length, 'files from stack', sourceStackId, 'onto', targetStackId);

      // Don't drop onto the same stack (unless moving to/from unassigned)
      if (targetStackId === sourceStackId && targetStackId !== 'unassigned' && sourceStackId !== '') {
        console.log('[GitButler DnD] same stack, ignoring');
        currentDropTarget = null; dragData = null; return;
      }

      if (dragData.paths.length === 1) {
        if (targetStackId === 'unassigned') {
          send({ type: 'unstage', filePath: dragData.paths[0] });
        } else {
          send({ type: 'moveToBranch', filePath: dragData.paths[0], targetStackId });
        }
      } else {
        if (targetStackId === 'unassigned') {
          send({ type: 'unstageFiles', filePaths: dragData.paths });
        } else {
          send({ type: 'moveFiles', filePaths: dragData.paths, targetStackId });
        }
      }
      currentDropTarget = null;
      dragData = null;
    });

    // Listen for messages from the extension (e.g. AI-generated commit messages)
    window.addEventListener('message', (event) => {
      const msg = event.data;
      if (msg.type === 'setCommitMessage') {
        const input = document.querySelector('.commit-input[data-stack-id="' + msg.stackId + '"]');
        if (input) {
          input.value = msg.message;
          input.style.height = 'auto';
          input.style.height = Math.min(input.scrollHeight, 80) + 'px';
          input.focus();
        }
      }
    });
  </script>
</body>
</html>`;
  }

  private renderBranches(stacks: Stack[], stagedChanges: FileChange[]): string {
    if (stacks.length === 0) return '';

    return stacks.map((stack) => {
      if (!stack.id) return '';
      const segments = stack.segments.filter((s) => s.refName);
      const files = stagedChanges.filter((c) => c.stackId === stack.id);
      // The first segment (index 0) is the tip — that's where new commits go
      // and where uncommitted worktree changes should be shown.

      return segments.map((seg, segIdx) => {
        const name = seg.refName?.displayName || '(unnamed)';
        const pushIcon = this.pushStatusIcon(seg.pushStatus);
        const isTip = segIdx === 0;
        // Only show files & commit input on the tip branch
        const segFiles = isTip ? files : [];

        return `
          <div class="section" data-drop-stack-id="${stack.id}">
            <div class="section-header">
              <span class="chevron codicon codicon-chevron-down"></span>
              ${pushIcon} ${this.esc(name)}
              ${!isTip ? '<span class="push-status">(base)</span>' : ''}
              <span class="push-status">${seg.pushStatus === 'completelyUnpushed' ? '(new)' : ''}</span>
              <div class="actions">
                <button onclick="event.stopPropagation(); send({type:'pushBranch', stackId:'${stack.id}', branchName:'${this.escJs(name)}'})" title="Push Branch"><span class="codicon codicon-cloud-upload"></span></button>
                <button onclick="send({type:'refresh'})" title="Refresh"><span class="codicon codicon-refresh"></span></button>
                <button onclick="event.stopPropagation(); send({type:'deleteBranch', stackId:'${stack.id}', branchName:'${this.escJs(name)}'})" title="Delete Branch"><span class="codicon codicon-trash"></span></button>
              </div>
              ${segFiles.length > 0 ? `<span class="badge">${segFiles.length}</span>` : ''}
            </div>
            <div class="section-body">
              ${isTip ? `<div class="commit-area">
                <div class="commit-input-wrap">
                  <textarea class="commit-input" rows="1"
                    placeholder="Message (Ctrl+Enter to commit on &quot;${this.esc(name)}&quot;)"
                    data-stack-id="${stack.id}"
                    data-branch-name="${this.esc(name)}"></textarea>
                  <button class="ai-btn" onclick="event.stopPropagation(); send({type:'generateCommitMessage', stackId:'${stack.id}', branchName:'${this.escJs(name)}'})" title="Generate commit message with AI"><span class="codicon codicon-sparkle"></span></button>
                </div>
                <button class="commit-btn" onclick="
                  const input = this.closest('.commit-area').querySelector('.commit-input');
                  const msg = input.value.trim();
                  if (msg) { send({type:'commit', stackId:'${stack.id}', branchName:'${this.esc(name)}', message: msg}); input.value=''; }
                "><span class="codicon codicon-check"></span> Commit</button>
              </div>` : ''}
              ${segFiles.length > 0 ? this.renderFileTree(segFiles, stack.id!) : (isTip ? '<div class="empty-msg">No assigned files. Drag files here.</div>' : '')}
              ${seg.commits.length > 0 ? this.renderCommits(seg) : ''}
            </div>
          </div>`;
      }).join('');
    }).join('');
  }

  private renderUnassigned(changes: FileChange[], branchData: { stackId: string; name: string }[]): string {
    return `
      <div class="section" data-drop-stack-id="unassigned">
        <div class="section-header">
          <span class="chevron codicon codicon-chevron-down"></span>
          Unassigned Changes
          ${changes.length > 0 ? `<span class="badge">${changes.length}</span>` : ''}
        </div>
        <div class="section-body">
          ${changes.length > 0 ? this.renderUnassignedFileTree(changes, branchData) : '<div class="empty-msg">No unassigned changes</div>'}
        </div>
      </div>`;
  }

  private renderFileTree(files: FileChange[], stackId: string): string {
    const tree = this.buildTree(files);
    const allPaths = this.collectAllPaths(tree);
    const allPathsJson = JSON.stringify(allPaths).replace(/"/g, '&quot;');
    const rootActions = `<div class="file-actions"><button onclick="event.stopPropagation(); send({type:'unstageFiles', filePaths:JSON.parse(this.closest('.folder-item').dataset.paths)})" title="Unassign all"><span class="codicon codicon-remove"></span></button></div>`;
    const rootHandle = files.length > 1
      ? `<div class="folder-item root-drag" draggable="true" data-paths="${allPathsJson}" data-stack-id="${stackId}">
           <span class="icon codicon codicon-folder-opened"></span> <span class="name">Changes</span>
           <span class="badge" style="margin-left:auto">${files.length}</span>
           ${rootActions}
         </div>`
      : '';
    return `<div class="file-tree">${rootHandle}${this.renderTreeNodes(tree, stackId, false)}</div>`;
  }

  private renderUnassignedFileTree(files: FileChange[], branchData: { stackId: string; name: string }[]): string {
    const tree = this.buildTree(files);
    const allPaths = this.collectAllPaths(tree);
    const allPathsJson = JSON.stringify(allPaths).replace(/"/g, '&quot;');
    let rootActions = '';
    if (branchData && branchData.length > 0) {
      if (branchData.length === 1) {
        rootActions = `<div class="file-actions"><button onclick="event.stopPropagation(); send({type:'stageFiles', filePaths:JSON.parse(this.closest('.folder-item').dataset.paths), stackId:'${branchData[0].stackId}'})" title="Assign all to ${this.esc(branchData[0].name)}"><span class="codicon codicon-add"></span></button></div>`;
      } else {
        const btns = branchData.map((b) =>
          `<button onclick="event.stopPropagation(); send({type:'stageFiles', filePaths:JSON.parse(this.closest('.folder-item').dataset.paths), stackId:'${b.stackId}'})" title="Assign all to ${this.esc(b.name)}" style="font-size:11px"><span class="codicon codicon-arrow-right"></span>${this.esc(b.name.substring(0, 8))}</button>`
        ).join('');
        rootActions = `<div class="file-actions">${btns}</div>`;
      }
    }
    const rootHandle = files.length > 1
      ? `<div class="folder-item root-drag" draggable="true" data-paths="${allPathsJson}" data-stack-id="">
           <span class="icon codicon codicon-folder-opened"></span> <span class="name">All Changes</span>
           <span class="badge" style="margin-left:auto">${files.length}</span>
           ${rootActions}
         </div>`
      : '';
    return `<div class="file-tree">${rootHandle}${this.renderTreeNodes(tree, '', true, branchData)}</div>`;
  }

  private buildTree(files: FileChange[]): TreeNode[] {
    const root: Map<string, TreeNode> = new Map();

    for (const file of files) {
      const parts = file.path.split('/');
      let currentLevel = root;

      for (let i = 0; i < parts.length; i++) {
        const part = parts[i];
        const isFile = i === parts.length - 1;

        if (isFile) {
          currentLevel.set(part, { name: part, file, children: null });
        } else {
          let node = currentLevel.get(part);
          if (!node) {
            node = { name: part, file: null, children: new Map() };
            currentLevel.set(part, node);
          }
          currentLevel = node.children!;
        }
      }
    }

    return this.flattenTree(root);
  }

  private flattenTree(nodes: Map<string, TreeNode>): TreeNode[] {
    const result: TreeNode[] = [];
    const sorted = Array.from(nodes.entries()).sort(([a], [b]) => {
      const aIsDir = nodes.get(a)?.children !== null;
      const bIsDir = nodes.get(b)?.children !== null;
      if (aIsDir && !bIsDir) return -1;
      if (!aIsDir && bIsDir) return 1;
      return a.localeCompare(b);
    });
    for (const [, node] of sorted) {
      result.push(node);
    }
    return result;
  }

  private renderTreeNodes(
    nodes: TreeNode[],
    stackId: string,
    isUnassigned: boolean,
    branchData?: { stackId: string; name: string }[]
  ): string {
    return nodes.map((node) => {
      if (node.children) {
        const children = this.flattenTree(node.children);
        const folderPaths = this.collectPaths(node);
        const folderPathsJson = JSON.stringify(folderPaths).replace(/"/g, '&quot;');
        const folderPathsJs = JSON.stringify(folderPaths).replace(/'/g, "\\'");

        let folderActions = '';
        if (isUnassigned) {
          if (branchData && branchData.length > 0) {
            if (branchData.length === 1) {
              folderActions = `<button onclick="event.stopPropagation(); send({type:'stageFiles', filePaths:JSON.parse(this.closest('.folder-item').dataset.paths), stackId:'${branchData[0].stackId}'})" title="Assign all to ${this.esc(branchData[0].name)}"><span class="codicon codicon-add"></span></button>`;
            } else {
              folderActions = branchData.map((b) =>
                `<button onclick="event.stopPropagation(); send({type:'stageFiles', filePaths:JSON.parse(this.closest('.folder-item').dataset.paths), stackId:'${b.stackId}'})" title="Assign all to ${this.esc(b.name)}" style="font-size:11px"><span class="codicon codicon-arrow-right"></span>${this.esc(b.name.substring(0, 8))}</button>`
              ).join('');
            }
          }
        } else {
          folderActions = `<button onclick="event.stopPropagation(); send({type:'unstageFiles', filePaths:JSON.parse(this.closest('.folder-item').dataset.paths)})" title="Unassign all"><span class="codicon codicon-remove"></span></button>`;
        }

        return `
          <div class="folder-item" draggable="true" data-paths="${folderPathsJson}" data-stack-id="${stackId}">
            <span class="chevron codicon codicon-chevron-down"></span>
            <span class="codicon codicon-folder"></span> ${this.esc(node.name)}
            <span class="badge" style="margin-left:auto">${folderPaths.length}</span>
            <div class="file-actions">${folderActions}</div>
          </div>
          <div class="folder-children">
            ${this.renderTreeNodes(children, stackId, isUnassigned, branchData)}
          </div>`;
      }

      const file = node.file!;
      const statusClass = 'status-' + file.type;
      const statusLetter = this.statusLetter(file.type);

      let actions = '';
      if (isUnassigned) {
        // Stage buttons — one per branch
        if (branchData && branchData.length > 0) {
          if (branchData.length === 1) {
            actions = `<button onclick="event.stopPropagation(); send({type:'stage', filePath:'${this.escJs(file.path)}', stackId:'${branchData[0].stackId}'})" title="Assign to ${this.esc(branchData[0].name)}"><span class="codicon codicon-add"></span></button>`;
          } else {
            // Show first branch as default, right-click for others
            actions = branchData.map((b) =>
              `<button onclick="event.stopPropagation(); send({type:'stage', filePath:'${this.escJs(file.path)}', stackId:'${b.stackId}'})" title="Assign to ${this.esc(b.name)}" style="font-size:11px"><span class="codicon codicon-arrow-right"></span>${this.esc(b.name.substring(0, 8))}</button>`
            ).join('');
          }
        }
        actions += `<button onclick="event.stopPropagation(); send({type:'discard', filePath:'${this.escJs(file.path)}'})" title="Discard"><span class="codicon codicon-discard"></span></button>`;
      } else {
        actions = `
          <button onclick="event.stopPropagation(); send({type:'unstage', filePath:'${this.escJs(file.path)}'})" title="Unassign"><span class="codicon codicon-remove"></span></button>
          <button onclick="event.stopPropagation(); send({type:'discard', filePath:'${this.escJs(file.path)}'})" title="Discard"><span class="codicon codicon-discard"></span></button>`;
      }

      return `
        <div class="file-item ${statusClass}" draggable="true"
             data-path="${this.esc(file.path)}"
             data-stack-id="${stackId}"
             onclick="send({type:'openFile', filePath:'${this.escJs(file.path)}'})">
          <span class="icon ${statusClass}">${statusLetter}</span>
          <span class="name">${this.esc(node.name)}</span>
          <div class="file-actions">${actions}</div>
        </div>`;
    }).join('');
  }

  private renderCommits(segment: Segment): string {
    if (segment.commits.length === 0) return '';
    const commitHtml = segment.commits.map((c) => {
      const msg = c.message.split('\n')[0] || '';
      const sha = c.id.substring(0, 7);
      return `<div class="file-item" style="padding-left:20px;opacity:0.8">
        <span class="icon codicon codicon-git-commit"></span>
        <span class="name" style="font-size:12px">${this.esc(msg.substring(0, 60))}</span>
        <span style="color:var(--vscode-descriptionForeground);font-size:11px;margin-left:auto">${sha}</span>
      </div>`;
    }).join('');
    return `<details style="padding:0 4px"><summary style="padding:4px 8px;cursor:pointer;font-size:11px;color:var(--vscode-descriptionForeground)">
      ${segment.commits.length} commit${segment.commits.length !== 1 ? 's' : ''}
    </summary>${commitHtml}</details>`;
  }

  private pushStatusIcon(status: string): string {
    switch (status) {
      case 'nothingToPush': return '<span class="codicon codicon-check"></span>';
      case 'unpushedCommits': return '<span class="codicon codicon-cloud-upload"></span>';
      case 'unpushedCommitsRequiringForce': return '<span class="codicon codicon-warning"></span>';
      case 'completelyUnpushed': return '<span class="codicon codicon-circle-outline"></span>';
      case 'integrated': return '<span class="codicon codicon-check"></span>';
      default: return '<span class="codicon codicon-circle-outline"></span>';
    }
  }

  private statusLetter(type: string): string {
    switch (type) {
      case 'added': return 'A';
      case 'modified': return 'M';
      case 'deleted': return 'D';
      case 'renamed': return 'R';
      case 'untracked': return 'U';
      default: return '?';
    }
  }

  /** Collect all file paths under a tree node (recursively). */
  private collectPaths(node: TreeNode): string[] {
    if (node.file) return [node.file.path];
    if (!node.children) return [];
    const paths: string[] = [];
    for (const child of node.children.values()) {
      paths.push(...this.collectPaths(child));
    }
    return paths;
  }

  /** Collect all file paths from a list of tree nodes. */
  private collectAllPaths(nodes: TreeNode[]): string[] {
    const paths: string[] = [];
    for (const node of nodes) {
      paths.push(...this.collectPaths(node));
    }
    return paths;
  }

  private esc(str: string): string {
    return str.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
  }

  private escJs(str: string): string {
    return str.replace(/\\/g, '\\\\').replace(/'/g, "\\'");
  }
}

interface TreeNode {
  name: string;
  file: FileChange | null;
  children: Map<string, TreeNode> | null;
}
