package models

// Remote Branch : A git remote branch
type RemoteBranch struct {
	Name       string
	RemoteName string
	// hash of the commit the branch points at
	CommitHash string
	// how long ago that commit was made, e.g. "2w", like Branch.Recency
	Recency string
}

func (r *RemoteBranch) URN() string {
	return "remoteBranch-" + r.FullName()
}

func (r *RemoteBranch) FullName() string {
	return r.RemoteName + "/" + r.Name
}

func (r *RemoteBranch) FullRefName() string {
	return "refs/remotes/" + r.FullName()
}

func (r *RemoteBranch) RefName() string {
	return r.FullName()
}

func (r *RemoteBranch) ShortRefName() string {
	return r.RefName()
}

func (r *RemoteBranch) ParentRefName() string {
	return r.RefName() + "^"
}

func (r *RemoteBranch) ID() string {
	return r.RefName()
}

func (r *RemoteBranch) Description() string {
	return r.RefName()
}
