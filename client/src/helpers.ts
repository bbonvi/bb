import { useState } from "react";
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

export function deepClone<T>(obj: T): T {
    if (obj === null || typeof obj !== 'object') return obj;

    if (Array.isArray(obj)) {
        return obj.map(item => deepClone(item)) as unknown as T;
    }

    const cloned: any = {};
    for (const key in obj) {
        if (Object.prototype.hasOwnProperty.call(obj, key)) {
            cloned[key] = deepClone((obj as any)[key]);
        }
    }
    return cloned;
}

export type JsonValue = string | number | boolean | null | JsonValue[] | { [key: string]: JsonValue };

export type SerializableObject = { [key: string]: JsonValue };

export function useLocalStorage<T extends SerializableObject>(key: string, initialValue: T): [T, (value: T) => void] {
    const [storedValue, setStoredValue] = useState<T>(() => {
        try {
            const item = window.localStorage.getItem(key);
            return item ? JSON.parse(item) : initialValue;
        } catch (error) {
            return initialValue;
        }
    });

    const setValue = (value: T) => {
        try {
            const valueToStore = value instanceof Function ? value(storedValue) : value;
            setStoredValue(JSON.parse(JSON.stringify(valueToStore)));
            window.localStorage.setItem(key, JSON.stringify(valueToStore));
        } catch (error) {
            console.log(error);
        }
    };

    return [storedValue, setValue];
}
