/**
 * `TreeNode` → `vscode.TreeItem`. One provider for all five views.
 *
 * There is nothing to decide here, which is the point: every shape question was settled in
 * `model.ts` against plain data, and this file is the part that cannot be tested without an
 * editor. Keeping it free of judgement is what makes that acceptable.
 */

import * as vscode from 'vscode'

import { findByFile, parentIndex, type TreeNode } from './model.ts'

export class NodeTree implements vscode.TreeDataProvider<TreeNode> {
  private readonly emitter = new vscode.EventEmitter<TreeNode | undefined>()
  readonly onDidChangeTreeData = this.emitter.event
  private roots: TreeNode[] = []
  private parents = new Map<TreeNode, TreeNode>()

  constructor(private readonly root: () => string | null) {}

  replace(roots: TreeNode[]): void {
    this.roots = roots
    this.parents = parentIndex(roots)
    this.emitter.fire(undefined)
  }

  getChildren(element?: TreeNode): TreeNode[] {
    return element ? (element.children ?? []) : this.roots
  }

  /** Required by `TreeView.reveal`, which cannot select a nested row without it. */
  getParent(element: TreeNode): TreeNode | undefined {
    return this.parents.get(element)
  }

  /** The row standing for a repo-relative path, for callers that have a file and want a row. */
  find(file: string): TreeNode | undefined {
    return findByFile(this.roots, file)
  }

  getTreeItem(node: TreeNode): vscode.TreeItem {
    const collapsible =
      node.children && node.children.length > 0
        ? node.expanded
          ? vscode.TreeItemCollapsibleState.Expanded
          : vscode.TreeItemCollapsibleState.Collapsed
        : vscode.TreeItemCollapsibleState.None

    const item = new vscode.TreeItem(node.label, collapsible)
    item.id = node.id
    item.description = node.description
    item.tooltip = node.tooltip
    item.contextValue = node.context
    if (node.icon) item.iconPath = new vscode.ThemeIcon(node.icon)

    const base = this.root()
    if (node.file && base) {
      const uri = vscode.Uri.file(`${base}/${node.file}`)
      item.resourceUri = uri
      item.command = {
        command: 'vscode.open',
        title: 'Open',
        arguments: [
          uri,
          node.line
            ? { selection: new vscode.Range(node.line - 1, 0, node.line - 1, 0) }
            : undefined,
        ],
      }
    } else if (node.command) {
      item.command = {
        command: node.command.id,
        title: node.label,
        arguments: node.command.args ?? [],
      }
    }
    return item
  }
}
