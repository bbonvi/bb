import axios, { AxiosError } from 'axios';

const configHeaders = {
    "content-type": "application/json",
    "Accept": "application/json"
};

export interface Bmark {
    id: number,

    title: string,
    description: string,
    tags: string[],
    url: string,

    image_id?: string,
    icon_id?: string,

}

export interface SearchQuery {
    id?: number,
    title?: string,
    url?: string,
    description?: string,
    tags?: string,
    exact?: boolean,
    descending?: boolean,
}

export interface UpdateBmark {
    id: number;
    title?: string;
    description?: string;
    tags?: string;
    url?: string;
    image_b64?: string;
    icon_b64?: string;
}

function formatError(error: AxiosError) {
    let context = "";
    const errorfRomResponse = (error.response?.data as any)?.error;
    if (errorfRomResponse) {
        context += ": "
        context += errorfRomResponse.toString();
    } else if (error.response?.data && typeof error.response?.data === "string") {
        context += ": "
        context += error.response.data;
    } else if (error.response?.data && typeof error.response?.data === "object") {
        try {
            const str = JSON.stringify(error.response.data);
            context += ": "
            context += str
        } catch (err) {
            //
        }
    }

    throw Error(`${error.message}${context}`)
}

export function fetchBmarks(query: SearchQuery): Promise<Bmark[]> {
    return axios.post(
        "/api/bookmarks/search",
        query,
        { headers: configHeaders }
    )
        .then(resp => resp.data)
        .catch(formatError)
}

export interface BookmarkCreate {
    url: string;

    description?: string;
    title?: string;
    tags?: string;

    async_meta?: boolean;
    no_meta?: boolean;
    no_headless?: boolean;
}


export function createBmark(create: BookmarkCreate): Promise<Bmark> {
    return axios.post(
        "/api/bookmarks/create",
        create,
        { headers: configHeaders }
    )
        .then(resp => resp.data)
        .catch(formatError)
}


export function deleteBmark(id: number): Promise<any> {
    return axios.post(
        "/api/bookmarks/delete",
        { id },
        { headers: configHeaders }
    )
        .catch(formatError)
}

export function fetchTotal(): Promise<number> {
    return axios.post(
        "/api/bookmarks/total",
        {},
        { headers: configHeaders }
    )
        .then(resp => resp.data.total)
        .catch(formatError)
}

export function updateBmark(id: number, updateBmark: UpdateBmark): Promise<Bmark> {
    return axios.post(
        "/api/bookmarks/update",
        {
            ...updateBmark,
            id,
        },
        { headers: configHeaders }
    )
        .then(resp => resp.data)
        .catch(formatError)
}

export function fetchMeta(id: number): Promise<any> {
    return axios.post(
        "/api/bookmarks/refresh_metadata",
        {
            id,
            async_meta: true,
            no_headless: false,
        },
        { headers: configHeaders }
    )
        .then(resp => resp.data)
        .catch(formatError)
}

export function fetchTags(): Promise<string[]> {
    return axios.post(
        "/api/bookmarks/tags",
        {},
        { headers: configHeaders }
    )
        .then(resp => resp.data)
        .catch(formatError)
}
interface Action {
    UpdateBookmark: {
        title?: string,
        description?: string,
        tags?: string[],
    }
}

export interface Rule {
    url?: string,
    description?: string,
    title?: string,
    tags?: string[],
    comment?: string,
    action: Action,
}

export interface Config {
    task_queue_max_threads: number;
    hidden_by_default: string[];
    rules: Rule[];
}

export function fetchConfig(): Promise<Config> {
    return axios.get(
        "/api/config",
        { headers: configHeaders }
    )
        .then(resp => resp.data)
        .catch(formatError)
}

export interface Task {
    id: string;
    task: {
        FetchMetadata?:
        {
            "bmark_id": number,
            "opts": {
                "no_https_upgrade": false,
                "meta_opts": {
                    "no_headless": false
                }
            }
        }
    },
    status: "Interrupted" | "Pending" | "InProgress" | "Done" | { "Error": "couldnt't retrieve metadata" }
}

export interface TaskQueue {
    queue: Task[];
    now: number;
}

export function fetchTaskQueue(): Promise<TaskQueue> {
    return axios.get(
        "/api/task_queue",
        { headers: configHeaders }
    )
        .then(resp => resp.data)
        .catch(formatError)
}
