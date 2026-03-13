export type ChangeUnit =
	| {
			_tag: "commit";
			commitId: string;
	  }
	| {
			_tag: "changes";
			stackId: string | null;
	  };
