import { SerializableObject } from "./helpers";

export interface Workspace extends SerializableObject {
    name: string;
    tags: Tags;
}

export type Tags = {
    whitelist: string[];
    blacklist: string[];
}

export interface WorkspaceState extends SerializableObject {
    workspaces: Workspace[];
    currentWorkspace: number;
}

export function defaultWorkspace(): Workspace {
    return {
        name: "Default",
        tags: {
            blacklist: [],
            whitelist: [],
        },
    }
}
