export type SessionFile = {
    name: string;
    path: string;
    linesTouched: number;
    numberOfEdits: number;
};

export type ActivityType = "commit" | "merge" | "rebase" | "push";

export type SessionActivity = {
    timestamp: number;
    type: ActivityType;
};

export type Session = {
    hash: string;
    startTime: number;
    endTime: number;
    branchName: string;
    files: SessionFile[];
    activities: SessionActivity[];
};

// for testing and development only
export let dummySessions: Session[] = [
    {
        hash: "1-a1b2c3d4e5f6g7h8i9j0",
        startTime: Math.floor(new Date("2023-01-01T08:00:00.000Z").getTime()),
        endTime: Math.floor(new Date("2023-01-01T09:00:00.000Z").getTime()),
        branchName: "update-docs",
        files: [
            {
                name: "README.md",
                path: "/README.md",
                linesTouched: 12,
                numberOfEdits: 45,
            },
            {
                name: "index.ts",
                path: "/src/index.ts",
                linesTouched: 3,
                numberOfEdits: 8,
            },
        ],
        activities: [
            {
                timestamp: Math.floor(new Date("2023-01-01T08:01:00.000Z").getTime()),
                type: "commit",
            },
            {
                timestamp: Math.floor(new Date("2023-01-01T08:59:00.000Z").getTime()),
                type: "push",
            },
        ],
    },
    {
        hash: "2-a1b2c3d4e5f6g7h8i9j0",
        startTime: Math.floor(new Date("2023-01-01T14:00:00.000Z").getTime()),
        endTime: Math.floor(new Date("2023-01-01T15:30:00.000Z").getTime()),
        branchName: "newer-dependencies",
        files: [
            {
                name: "package.json",
                path: "package.json",
                linesTouched: 4,
                numberOfEdits: 15,
            },
            {
                name: "tailwind.config.cjs",
                path: "tailwind.config.cjs",
                linesTouched: 23,
                numberOfEdits: 92,
            },
        ],
        activities: [
            {
                timestamp: Math.floor(new Date("2023-01-01T14:10:00.000Z").getTime()),
                type: "rebase",
            },
            {
                timestamp: Math.floor(new Date("2023-01-01T14:30:00.000Z").getTime()),
                type: "commit",
            },
            {
                timestamp: Math.floor(new Date("2023-01-01T14:40:00.000Z").getTime()),
                type: "push",
            },
        ],
    },
];
