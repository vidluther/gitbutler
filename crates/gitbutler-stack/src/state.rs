use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Result};
use git2::Repository;
use gitbutler_error::error::Code;
use gitbutler_fs::read_toml_file_or_default;
// use gitbutler_project::Project;
use gitbutler_reference::Refname;
use itertools::Itertools;
use serde::{Deserialize, Serialize};

use crate::{
    stack::{Stack, StackId},
    target::Target,
};

/// The state of virtual branches data, as persisted in a TOML file.
#[derive(Serialize, Deserialize, Debug, Default)]
pub struct VirtualBranches {
    /// This is the target/base that is set when a repo is added to gb
    default_target: Option<Target>,
    /// The targets for each virtual branch
    branch_targets: HashMap<StackId, Target>,
    /// The current state of the virtual branches
    branches: HashMap<StackId, Stack>,
}

impl VirtualBranches {
    /// Lists all virtual branches that are in the user's workspace.
    ///
    /// Errors if the file cannot be read or written.
    pub(crate) fn list_all_branches(&self) -> Result<Vec<Stack>> {
        let branches: Vec<Stack> = self.branches.values().cloned().collect();
        Ok(branches)
    }

    /// Lists all virtual branches that are in the user's workspace.
    ///
    /// Errors if the file cannot be read or written.
    pub fn list_branches_in_workspace(&self) -> Result<Vec<Stack>> {
        self.list_all_branches().map(|branches| {
            branches
                .into_iter()
                .filter(|branch| branch.in_workspace)
                .collect()
        })
    }
}

/// A handle to the state of virtual branches.
///
/// For all operations, if the state file does not exist, it will be created.
pub struct VirtualBranchesHandle {
    /// The path to the file containing the virtual branches state.
    file_path: PathBuf,
}

// pub trait VirtualBranchesExt {
//     fn virtual_branches(&self) -> VirtualBranchesHandle;
// }

// impl VirtualBranchesExt for Project {
//     fn virtual_branches(&self) -> VirtualBranchesHandle {
//         VirtualBranchesHandle::new(self.gb_dir())
//     }
// }

impl VirtualBranchesHandle {
    /// Creates a new concurrency-safe handle to the state of virtual branches.
    pub fn new<P: AsRef<Path>>(base_path: P) -> Self {
        let file_path = base_path.as_ref().join("virtual_branches.toml");
        Self { file_path }
    }

    /// Persists the default target for the given repository.
    ///
    /// Errors if the file cannot be read or written.
    pub fn set_default_target(&self, target: Target) -> Result<()> {
        let mut virtual_branches = self.read_file()?;
        virtual_branches.default_target = Some(target);
        self.write_file(&virtual_branches)?;
        Ok(())
    }

    /// Gets the default target for the given repository.
    ///
    /// Errors if the file cannot be read or written.
    pub fn get_default_target(&self) -> Result<Target> {
        let virtual_branches = self.read_file()?;
        virtual_branches
            .default_target
            .ok_or(anyhow!("there is no default target").context(Code::DefaultTargetNotFound))
    }

    /// Sets the state of the given virtual branch.
    ///
    /// Errors if the file cannot be read or written.
    pub fn set_branch(&self, branch: Stack) -> Result<()> {
        let mut virtual_branches = self.read_file()?;
        virtual_branches.branches.insert(branch.id, branch);
        self.write_file(&virtual_branches)?;
        Ok(())
    }

    /// Marks a particular branch as not in the workspace
    ///
    /// Errors if the file cannot be read or written.
    pub fn mark_as_not_in_workspace(&self, id: StackId) -> Result<()> {
        let mut branch = self.get_branch(id)?;
        branch.in_workspace = false;
        self.set_branch(branch)?;
        Ok(())
    }

    pub fn find_by_source_refname_where_not_in_workspace(
        &self,
        refname: &Refname,
    ) -> Result<Option<Stack>> {
        let branches = self.list_all_branches()?;
        Ok(branches.into_iter().find(|branch| {
            if branch.in_workspace {
                return false;
            }

            if let Some(source_refname) = branch.source_refname.as_ref() {
                return source_refname.to_string() == refname.to_string();
            }

            false
        }))
    }

    /// Gets the state of the given virtual branch.
    ///
    /// Errors if the file cannot be read or written.
    pub fn get_branch_in_workspace(&self, id: StackId) -> Result<Stack> {
        self.try_branch_in_workspace(id)?
            .ok_or_else(|| anyhow!("branch with ID {id} not found"))
    }

    /// Gets the state of the given virtual branch.
    ///
    /// Errors if the file cannot be read or written.
    pub fn get_branch(&self, id: StackId) -> Result<Stack> {
        self.try_branch(id)?
            .ok_or_else(|| anyhow!("branch with ID {id} not found"))
    }

    /// Gets the state of the given virtual branch returning `Some(branch)` or `None`
    /// if that branch doesn't exist.
    pub fn try_branch_in_workspace(&self, id: StackId) -> Result<Option<Stack>> {
        Ok(self.try_branch(id)?.filter(|branch| branch.in_workspace))
    }

    /// Gets the state of the given virtual branch returning `Some(branch)` or `None`
    /// if that branch doesn't exist.
    pub fn try_branch(&self, id: StackId) -> Result<Option<Stack>> {
        let virtual_branches = self.read_file()?;
        Ok(virtual_branches.branches.get(&id).cloned())
    }

    /// Lists all branches in `virtual_branches.toml`.
    ///
    /// Errors if the file cannot be read or written.
    pub fn list_all_branches(&self) -> Result<Vec<Stack>> {
        let virtual_branches = self.read_file()?;
        let branches: Vec<Stack> = virtual_branches.branches.values().cloned().collect();
        Ok(branches)
    }

    /// Lists all virtual branches that are in the user's workspace.
    ///
    /// Errors if the file cannot be read or written.
    pub fn list_branches_in_workspace(&self) -> Result<Vec<Stack>> {
        self.list_all_branches().map(|branches| {
            branches
                .into_iter()
                .filter(|branch| branch.in_workspace)
                .collect()
        })
    }

    /// Reads and parses the state file.
    ///
    /// If the file does not exist, it will be created.
    fn read_file(&self) -> Result<VirtualBranches> {
        read_toml_file_or_default(&self.file_path)
    }

    fn write_file(&self, virtual_branches: &VirtualBranches) -> Result<()> {
        write(self.file_path.as_path(), virtual_branches)
    }

    pub fn update_ordering(&self) -> Result<()> {
        let succeeded = self
            .list_branches_in_workspace()?
            .iter()
            .sorted_by_key(|branch| branch.order)
            .enumerate()
            .all(|(index, branch)| {
                let mut branch = branch.clone();
                branch.order = index;
                self.set_branch(branch).is_ok()
            });

        if succeeded {
            Ok(())
        } else {
            Err(anyhow!("Failed to update virtual branches ordering"))
        }
    }

    pub fn next_order_index(&self) -> Result<usize> {
        self.update_ordering()?;
        let order = self
            .list_branches_in_workspace()?
            .iter()
            .sorted_by_key(|branch| branch.order)
            .collect::<Vec<&Stack>>()
            .last()
            .map_or(0, |b| b.order + 1);

        Ok(order)
    }

    pub fn delete_branch_entry(&self, branch_id: &StackId) -> Result<()> {
        let mut virtual_branches = self.read_file()?;
        virtual_branches.branches.remove(branch_id);
        self.write_file(&virtual_branches)?;
        Ok(())
    }

    /// Garbage collects branches that are not in the workspace and hold no changes:
    ///   1. They do not have a WIP commit
    ///   2. They have no regular commits
    ///
    /// Also collects branches with a head oid pointing to a commit that can't be found in the repo
    pub fn garbage_collect(&self, repo: &Repository) -> Result<()> {
        let target = self.get_default_target()?;
        let branches_not_in_workspace = self
            .list_all_branches()?
            .into_iter()
            .filter(|b| !b.in_workspace)
            .collect_vec();
        let mut to_remove: Vec<StackId> = vec![];
        for branch in branches_not_in_workspace {
            if branch.not_in_workspace_wip_change_id.is_some() {
                continue; // Skip branches that have a WIP commit
            }
            if repo.find_commit(branch.head()).is_err() {
                // if the head commit cant be found, we can GC the branch
                to_remove.push(branch.id);
            } else {
                // if there are no commits between the head and the merge base,
                // i.e. the head is the merge base, we can GC the branch
                if branch.head() == repo.merge_base(branch.head(), target.sha)? {
                    to_remove.push(branch.id);
                }
            }
        }
        if !to_remove.is_empty() {
            let mut virtual_branches = self.read_file()?;
            for branch_id in to_remove {
                virtual_branches.branches.remove(&branch_id);
            }
            // Perform all removals in one go (Windows doesn't like multiple writes in quick succession)
            self.write_file(&virtual_branches)?;
        }

        Ok(())
    }
}

fn write<P: AsRef<Path>>(file_path: P, virtual_branches: &VirtualBranches) -> Result<()> {
    gitbutler_fs::write(file_path, toml::to_string(&virtual_branches)?)
}