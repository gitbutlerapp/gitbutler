export type FileParent =
	| {
			_tag: "Commit";
			commitId: string;
	  }
	| {
			_tag: "Changes";
			stackId: string | null;
	  };
