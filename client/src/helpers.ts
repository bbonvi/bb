import store from "./store";

export const isModKey = (e: React.KeyboardEvent<any> | KeyboardEvent) => {
    return e.metaKey || e.ctrlKey || e.altKey || e.shiftKey;
}

export function findTask(id: number) {
    const taskQueue = store.getState().taskQueue.value.queue;
    const queue = [...taskQueue]
    queue.reverse();
    const currTask = queue.find(tq => tq.task.FetchMetadata?.bmark_id === id);
    return currTask
}

export function findRunningTask(id: number) {
    const taskQueue = store.getState().taskQueue.value.queue;
    const queue = [...taskQueue]
    queue.reverse();
    const currTask = queue.find(tq => tq.task.FetchMetadata?.bmark_id === id && tq.status !== "Done");
    return currTask
}

export const toBase64 = (file: File): Promise<string> => new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.readAsDataURL(file);
    reader.onload = () => resolve((reader.result as string).split(',')[1]);
    reader.onerror = reject;
});
