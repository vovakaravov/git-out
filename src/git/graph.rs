use super::Commit;

/// A line or bezier curve connecting `(from_col, top_or_mid)` to `(to_col, mid_or_bottom)` in a row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphEdge {
    pub from_col: usize,
    pub to_col: usize,
    pub color: usize,
}

/// Visual layout for a single commit row in the rail graph.
#[derive(Clone, Debug)]
pub struct GraphRow {
    pub commit_col: usize,
    pub commit_color: usize,
    pub lane_count: usize,
    pub is_merge: bool,
    pub is_head: bool,
    /// Edges in the top half of the row (`y: 0 -> mid_y`), terminating at `(commit_col, mid_y)`.
    pub top_edges: Vec<GraphEdge>,
    /// Edges in the bottom half of the row (`y: mid_y -> row_h`), starting at `(commit_col, mid_y)`.
    pub bottom_edges: Vec<GraphEdge>,
    /// Straight vertical rails passing through the entire row (`y: 0 -> row_h`) as `(col, color)`.
    pub pass_through: Vec<(usize, usize)>,
}

/// Computes the GitKraken-style rail layout for commits ordered top-to-bottom (`git log --topo-order`).
pub fn compute_graph(commits: &[Commit]) -> Vec<GraphRow> {
    let mut rows = Vec::with_capacity(commits.len());
    // Active rails: each occupied slot holds `(expected_parent_hash, color_idx)`.
    let mut lanes: Vec<Option<(String, usize)>> = Vec::new();
    let mut next_color: usize = 0;

    for commit in commits {
        let lanes_before = lanes.len();

        // 1. Find all lanes currently expecting this commit's hash.
        let matching_cols: Vec<usize> = lanes
            .iter()
            .enumerate()
            .filter_map(|(idx, slot)| {
                slot.as_ref()
                    .and_then(|(hash, _)| (hash == &commit.hash).then_some(idx))
            })
            .collect();

        let mut top_edges = Vec::new();
        let commit_col: usize;
        let commit_color: usize;

        if let Some(&first_col) = matching_cols.first() {
            commit_col = first_col;
            commit_color = lanes[first_col].as_ref().map(|(_, c)| *c).unwrap_or(0);

            // Primary incoming rail down to the commit node.
            top_edges.push(GraphEdge {
                from_col: commit_col,
                to_col: commit_col,
                color: commit_color,
            });

            // Additional converging rails curve into `commit_col` and release their lanes.
            for &extra_col in matching_cols.iter().skip(1) {
                let extra_color = lanes[extra_col]
                    .as_ref()
                    .map(|(_, c)| *c)
                    .unwrap_or(commit_color);
                top_edges.push(GraphEdge {
                    from_col: extra_col,
                    to_col: commit_col,
                    color: extra_color,
                });
                lanes[extra_col] = None;
            }
        } else {
            // Branch tip / new head not expected by any descendant.
            commit_color = next_color;
            next_color = next_color.wrapping_add(1);
            commit_col = allocate_lane(&mut lanes);
        }

        // 2. Collect pass-through rails (every occupied lane not converging on this commit).
        let mut pass_through = Vec::new();
        for (col, slot) in lanes.iter().enumerate() {
            if col == commit_col || matching_cols.contains(&col) {
                continue;
            }
            if let Some((_, color)) = slot {
                pass_through.push((col, *color));
            }
        }

        // Clear the commit's own incoming slot before assigning parent expectations.
        lanes[commit_col] = None;

        // 3. Route outgoing edges to parents in the bottom half of this row.
        let mut bottom_edges = Vec::new();

        if let Some(first_parent) = commit.parents.first() {
            let existing_left = lanes
                .iter()
                .enumerate()
                .find_map(|(idx, slot)| match slot {
                    Some((hash, color)) if hash == first_parent && idx < commit_col => {
                        Some((idx, *color))
                    }
                    _ => None,
                });

            if let Some((target_col, _)) = existing_left {
                // Collapse rightward branch leftward into the existing lane expecting `first_parent`.
                bottom_edges.push(GraphEdge {
                    from_col: commit_col,
                    to_col: target_col,
                    color: commit_color,
                });
            } else {
                // Keep first parent straight on `commit_col`.
                lanes[commit_col] = Some((first_parent.clone(), commit_color));
                bottom_edges.push(GraphEdge {
                    from_col: commit_col,
                    to_col: commit_col,
                    color: commit_color,
                });
            }
        }

        // Additional parents (merge commits) spawn or join secondary rails.
        for parent in commit.parents.iter().skip(1) {
            let existing = lanes.iter().enumerate().find_map(|(idx, slot)| match slot {
                Some((hash, color)) if hash == parent => Some((idx, *color)),
                _ => None,
            });

            if let Some((target_col, target_color)) = existing {
                bottom_edges.push(GraphEdge {
                    from_col: commit_col,
                    to_col: target_col,
                    color: target_color,
                });
            } else {
                let edge_color = next_color;
                next_color = next_color.wrapping_add(1);
                let slot = allocate_lane(&mut lanes);
                lanes[slot] = Some((parent.clone(), edge_color));
                bottom_edges.push(GraphEdge {
                    from_col: commit_col,
                    to_col: slot,
                    color: edge_color,
                });
            }
        }

        let lane_count = lanes_before.max(lanes.len()).max(commit_col + 1);

        // Trim trailing empty slots so lane widths stay compact.
        while lanes.last().is_some_and(|slot| slot.is_none()) {
            lanes.pop();
        }

        rows.push(GraphRow {
            commit_col,
            commit_color,
            lane_count,
            is_merge: commit.parents.len() > 1,
            is_head: commit.is_head,
            top_edges,
            bottom_edges,
            pass_through,
        });
    }

    rows
}

fn allocate_lane(lanes: &mut Vec<Option<(String, usize)>>) -> usize {
    if let Some(idx) = lanes.iter().position(|slot| slot.is_none()) {
        idx
    } else {
        lanes.push(None);
        lanes.len() - 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk_commit(hash: &str, parents: &[&str], is_head: bool) -> Commit {
        Commit {
            hash: hash.to_string(),
            short_hash: hash.chars().take(7).collect(),
            parents: parents.iter().map(|s| (*s).to_string()).collect(),
            refs: Vec::new(),
            author: "Test".to_string(),
            author_email: "test@example.com".to_string(),
            timestamp: 1_700_000_000,
            subject: format!("Commit {hash}"),
            is_head,
        }
    }

    #[test]
    fn linear_history_stays_in_lane_zero() {
        let commits = vec![
            mk_commit("c3", &["c2"], true),
            mk_commit("c2", &["c1"], false),
            mk_commit("c1", &[], false),
        ];
        let rows = compute_graph(&commits);
        assert_eq!(rows.len(), 3);

        assert_eq!(rows[0].commit_col, 0);
        assert!(rows[0].top_edges.is_empty());
        assert_eq!(rows[0].bottom_edges.len(), 1);
        assert_eq!(rows[0].bottom_edges[0].to_col, 0);
        assert!(rows[0].is_head);

        assert_eq!(rows[1].commit_col, 0);
        assert_eq!(rows[1].top_edges.len(), 1);
        assert_eq!(rows[1].bottom_edges.len(), 1);

        assert_eq!(rows[2].commit_col, 0);
        assert_eq!(rows[2].top_edges.len(), 1);
        assert!(rows[2].bottom_edges.is_empty());
    }

    #[test]
    fn branch_and_merge_diamond_opens_and_closes_lane() {
        // M merges B (first parent) and F (second parent); both descend from A.
        let commits = vec![
            mk_commit("M", &["B", "F"], true),
            mk_commit("F", &["A"], false),
            mk_commit("B", &["A"], false),
            mk_commit("A", &[], false),
        ];
        let rows = compute_graph(&commits);
        assert_eq!(rows.len(), 4);

        // M is at col 0 and forks a second lane to col 1 for F.
        assert_eq!(rows[0].commit_col, 0);
        assert!(rows[0].is_merge);
        assert_eq!(rows[0].bottom_edges.len(), 2);
        assert_eq!(rows[0].bottom_edges[0].to_col, 0);
        assert_eq!(rows[0].bottom_edges[1].to_col, 1);

        // F is at col 1 while col 0 (expecting B) passes straight through.
        assert_eq!(rows[1].commit_col, 1);
        assert_eq!(rows[1].pass_through.len(), 1);
        assert_eq!(rows[1].pass_through[0].0, 0);

        // B is at col 0 while col 1 (expecting A) passes through.
        assert_eq!(rows[2].commit_col, 0);
        assert_eq!(rows[2].pass_through.len(), 1);
        assert_eq!(rows[2].pass_through[0].0, 1);

        // A converges col 1 back into col 0.
        assert_eq!(rows[3].commit_col, 0);
        assert_eq!(rows[3].top_edges.len(), 2);
        assert!(rows[3].bottom_edges.is_empty());
    }

    #[test]
    fn converging_unmerged_branches_collapse_cleanly() {
        // Two independent tips T1 and T2 both share parent Base.
        let commits = vec![
            mk_commit("T1", &["Base"], true),
            mk_commit("T2", &["Base"], false),
            mk_commit("Base", &[], false),
        ];
        let rows = compute_graph(&commits);
        assert_eq!(rows[0].commit_col, 0);
        assert_eq!(rows[1].commit_col, 1);
        // T2 sees Base is already expected at col 0, so it collapses leftward in bottom_edges.
        assert_eq!(rows[1].bottom_edges.len(), 1);
        assert_eq!(rows[1].bottom_edges[0].from_col, 1);
        assert_eq!(rows[1].bottom_edges[0].to_col, 0);
        // Base is back on col 0 with single lane.
        assert_eq!(rows[2].commit_col, 0);
        assert_eq!(rows[2].lane_count, 1);
    }
}
