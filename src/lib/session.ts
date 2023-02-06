export class SessionFile {
    name: string;
    path: string;
    linesTouched: number;
    numberOfEdits: number;
    constructor(name: string, path: string, linesTouched: number, numberOfEdits: number) {
        this.name = name;
        this.path = path;
        this.linesTouched = linesTouched;
        this.numberOfEdits = numberOfEdits;
    }
}
export enum ActivityType {
    COMMIT = "commit",
    MERGE = "merge",
    REBASE = "rebase",
    PUSH = "push",
}

export class SessionActivity {
    timestamp: number;
    type: ActivityType;
    constructor(timestamp: number, type: ActivityType) {
        this.timestamp = timestamp;
        this.type = type;
    }
}

export class Session {
    hash: string;
    startTime: number;
    endTime: number;
    branchName: string;
    files: SessionFile[];
    activities: SessionActivity[];
    constructor(hash: string, startTime: number, endTime: number, branchName: string, files: SessionFile[], activities: SessionActivity[]) {
        this.startTime = startTime;
        this.endTime = endTime;
        this.branchName = branchName;
        this.files = files;
        this.activities = activities;
        this.hash = hash;
    }
}

// for testing and development only
export let dummySessions: Session[] = [
    new Session(
        "1-a1b2c3d4e5f6g7h8i9j0",
        Math.floor(new Date('2023-01-01T08:00:00.000Z').getTime()),
        Math.floor(new Date('2023-01-01T09:00:00.000Z').getTime()),
        "update-docs",
        [
            new SessionFile("README.md", "/README.md", 12, 45),
            new SessionFile("index.ts", "/src/index.ts", 3, 8),
        ],
        [
            new SessionActivity(
                Math.floor(new Date('2023-01-01T08:30:00.000Z').getTime()),
                ActivityType.COMMIT),
            new SessionActivity(
                Math.floor(new Date('2023-01-01T08:40:00.000Z').getTime()),
                ActivityType.PUSH),
        ]
    ),
    new Session(
        "2-a1b2c3d4e5f6g7h8i9j0",
        Math.floor(new Date('2023-01-01T14:00:00.000Z').getTime()),
        Math.floor(new Date('2023-01-01T15:00:00.000Z').getTime()),
        "newer-dependencies",
        [
            new SessionFile("package.json", "package.json", 4, 15),
            new SessionFile("tailwind.config.cjs", "tailwind.config.cjs", 23, 92),
        ],
        [
            new SessionActivity(
                Math.floor(new Date('2023-01-01T14:10:00.000Z').getTime()),
                ActivityType.REBASE),
            new SessionActivity(
                Math.floor(new Date('2023-01-01T14:30:00.000Z').getTime()),
                ActivityType.COMMIT),
            new SessionActivity(
                Math.floor(new Date('2023-01-01T14:40:00.000Z').getTime()),
                ActivityType.PUSH),
        ]
    ),
];

