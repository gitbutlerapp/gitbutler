export type ChangeUnit =
	| {
			_tag: "Commit";
			commitId: string;
	  }
	| {
			_tag: "Changes";
			stackId: string | null;
	  };
