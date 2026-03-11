use super::types::{LayoutNode, LayoutRatios, PanelId, SplitDirection};
use super::LayoutManager;

impl LayoutManager {
    pub(crate) fn default_layout() -> LayoutNode {
        LayoutNode::Split {
            direction: SplitDirection::Horizontal,
            children: vec![
                (0.22, LayoutNode::Leaf(PanelId::Sidebar)),
                (0.78, LayoutNode::Leaf(PanelId::TerminalPane)),
            ],
        }
    }

    #[allow(dead_code)]
    fn default_layout_with_shell() -> LayoutNode {
        LayoutNode::Split {
            direction: SplitDirection::Vertical,
            children: vec![
                (
                    0.67,
                    LayoutNode::Split {
                        direction: SplitDirection::Horizontal,
                        children: vec![
                            (0.22, LayoutNode::Leaf(PanelId::Sidebar)),
                            (0.78, LayoutNode::Leaf(PanelId::TerminalPane)),
                        ],
                    },
                ),
                (0.33, LayoutNode::Leaf(PanelId::IntegratedTerminal)),
            ],
        }
    }

    pub(crate) fn rebuild_tree(&mut self) {
        self.layout_dirty = true;
        let sidebar = self.panel_visible[0];
        let terminal = self.panel_visible[1];
        let neovim = self.panel_visible[2];
        let shell = self.panel_visible[3];
        let git_panel = self.panel_visible[4];

        let ratios = self.extract_ratios();

        if neovim && shell {
            // Neovim + Shell visible: neovim is full-height on the right,
            // left side has top row (sidebar | opencode) over shell.
            //
            // ┌──────────┬──────────────┬───────────┐
            // │ Sidebar  │  Opencode    │           │
            // │          │              │  Neovim   │
            // ├──────────┴──────────────┤ (full     │
            // │                         │  height)  │
            // │  Terminal               │           │
            // └─────────────────────────┴───────────┘

            // Build top-left row: [sidebar | opencode | git]
            let mut top_left: Vec<(f64, LayoutNode)> = Vec::new();
            if sidebar {
                top_left.push((ratios.sidebar, LayoutNode::Leaf(PanelId::Sidebar)));
            }
            if terminal {
                top_left.push((ratios.terminal, LayoutNode::Leaf(PanelId::TerminalPane)));
            }
            if git_panel {
                top_left.push((ratios.git_panel, LayoutNode::Leaf(PanelId::GitPanel)));
            }

            let top_left_node = if top_left.len() == 1 {
                top_left.into_iter().next().unwrap().1
            } else if top_left.is_empty() {
                LayoutNode::Leaf(PanelId::TerminalPane)
            } else {
                LayoutNode::Split {
                    direction: SplitDirection::Horizontal,
                    children: top_left,
                }
            };

            // Left column: top-left row stacked over shell
            let left_column = LayoutNode::Split {
                direction: SplitDirection::Vertical,
                children: vec![
                    (ratios.top_vs_shell, top_left_node),
                    (
                        1.0 - ratios.top_vs_shell,
                        LayoutNode::Leaf(PanelId::IntegratedTerminal),
                    ),
                ],
            };

            // Full layout: left column | neovim
            // The left column takes (1 - neovim_ratio), neovim takes its ratio.
            let left_ratio = 1.0 - ratios.neovim;
            self.root = LayoutNode::Split {
                direction: SplitDirection::Horizontal,
                children: vec![
                    (left_ratio, left_column),
                    (ratios.neovim, LayoutNode::Leaf(PanelId::NeovimPane)),
                ],
            };
        } else {
            // No neovim+shell combo: flat layout.
            // Top row: [Sidebar | TerminalPane | NeovimPane | GitPanel]
            // Optionally stacked over IntegratedTerminal.
            let mut top_children: Vec<(f64, LayoutNode)> = Vec::new();
            if sidebar {
                top_children.push((ratios.sidebar, LayoutNode::Leaf(PanelId::Sidebar)));
            }
            if terminal {
                top_children.push((ratios.terminal, LayoutNode::Leaf(PanelId::TerminalPane)));
            }
            if neovim {
                top_children.push((ratios.neovim, LayoutNode::Leaf(PanelId::NeovimPane)));
            }
            if git_panel {
                top_children.push((ratios.git_panel, LayoutNode::Leaf(PanelId::GitPanel)));
            }

            let top = if top_children.len() == 1 {
                top_children.into_iter().next().unwrap().1
            } else if top_children.is_empty() {
                if shell {
                    self.root = LayoutNode::Leaf(PanelId::IntegratedTerminal);
                } else {
                    self.root = LayoutNode::Leaf(PanelId::TerminalPane);
                }
                return;
            } else {
                LayoutNode::Split {
                    direction: SplitDirection::Horizontal,
                    children: top_children,
                }
            };

            if shell {
                self.root = LayoutNode::Split {
                    direction: SplitDirection::Vertical,
                    children: vec![
                        (ratios.top_vs_shell, top),
                        (
                            1.0 - ratios.top_vs_shell,
                            LayoutNode::Leaf(PanelId::IntegratedTerminal),
                        ),
                    ],
                };
            } else {
                self.root = top;
            }
        }
    }

    pub(crate) fn extract_ratios(&self) -> LayoutRatios {
        let mut ratios = LayoutRatios {
            sidebar: 0.22,
            terminal: 0.78,
            neovim: 0.39,
            top_vs_shell: 0.67,
            git_panel: 0.39,
        };

        Self::extract_ratios_recursive(&self.root, &mut ratios);

        ratios
    }

    fn extract_ratios_recursive(node: &LayoutNode, ratios: &mut LayoutRatios) {
        match node {
            LayoutNode::Split {
                direction: SplitDirection::Horizontal,
                children,
            } => {
                for (r, child) in children {
                    match child {
                        LayoutNode::Leaf(PanelId::Sidebar) => ratios.sidebar = *r,
                        LayoutNode::Leaf(PanelId::TerminalPane) => ratios.terminal = *r,
                        LayoutNode::Leaf(PanelId::NeovimPane) => ratios.neovim = *r,
                        LayoutNode::Leaf(PanelId::GitPanel) => ratios.git_panel = *r,
                        // Left column in neovim+shell layout:
                        // V[ H[sidebar|opencode|git], Shell ]
                        LayoutNode::Split {
                            direction: SplitDirection::Vertical,
                            children: inner,
                        } => {
                            // top_vs_shell from this vertical split
                            if inner.len() == 2 {
                                ratios.top_vs_shell = inner[0].0;
                            }
                            // Recurse into the top row (horizontal) for sidebar/terminal/git
                            Self::extract_ratios_recursive(&inner[0].1, ratios);
                        }
                        _ => {}
                    }
                }
            }
            LayoutNode::Split {
                direction: SplitDirection::Vertical,
                children,
            } if children.len() == 2 => {
                // Original layout: top row over shell (no neovim)
                ratios.top_vs_shell = children[0].0;
                Self::extract_ratios_recursive(&children[0].1, ratios);
            }
            _ => {}
        }
    }
}
