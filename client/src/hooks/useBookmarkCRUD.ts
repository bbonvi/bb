import { useRef } from 'react';
import { useDispatch } from 'react-redux';
import { toast } from 'react-hot-toast';
import * as api from '../api';
import * as bmarksSlice from '../store/bmarksSlice';
import * as taskQueueSlice from '../store/taskQueueSlice';
import { BookmarkCreate, UpdateBmark } from '../api';
import { findRunningTask } from '../helpers';
import store from '../store';

interface UseBookmarkCRUDProps {
    setEditingId: (id: number) => void;
    setCreating: (val: boolean) => void;
    refreshTags: () => Promise<void>;
    refreshTotal: () => void;
}

export function useBookmarkCRUD({
    setEditingId,
    setCreating,
    refreshTags,
    refreshTotal,
}: UseBookmarkCRUDProps) {
    const dispatch = useDispatch();
    const updating = useRef(0);

    const handleDelete = (id: number) => {
        const currTask = findRunningTask(id);
        if (currTask) {
            toast.error("cannot delete while being processed");
            return;
        }

        updating.current += 1;
        toast.promise(api.deleteBmark(id).then(() => {
            dispatch(bmarksSlice.remove({ id }));
            setEditingId(-1);
            refreshTags();
            refreshTotal();
        }).finally(() => {
            updating.current -= 1;
        }), {
            loading: 'Deleting...',
            success: 'Deleted!',
            error: (err) => err.message,
        });
    };

    const handleFetchMeta = (id: number) => {
        const currTask = findRunningTask(id);
        if (currTask) {
            toast.error("cannot update while being processed.");
            return;
        }

        toast.promise(
            api.fetchMeta(id).then(() => {
                setEditingId(-1);
                refreshTaskQueue()
                    .then(() => setTimeout(refreshTaskQueue, 200));
            }),
            {
                loading: 'Requesting metadata refetch...',
                success: 'Requested metadata refetch!',
                error: (err) => err.message,
            }
        );
    };

    const handleSave = (update: UpdateBmark) => {
        const currTask = findRunningTask(update.id);
        if (currTask) {
            toast.error("cannot update while being processed.");
            return;
        }

        updating.current += 1;

        toast.promise(
            api.updateBmark(update.id, update).then((bmark) => {
                dispatch(bmarksSlice.update(bmark));
                setEditingId(-1);
                refreshTags();
            }).finally(() => {
                updating.current -= 1;
            }),
            {
                loading: 'Saving...',
                success: 'Saved!',
                error: (err) => err.message,
            }
        );
    };

    const onCreate = (bmark: BookmarkCreate) => {
        updating.current += 1;
        toast.promise(
            api.createBmark({
                ...bmark,
            }).then((bmark) => {
                dispatch(bmarksSlice.create(bmark));
                setCreating(false);
                setTimeout(() => {
                    refreshTaskQueue();
                }, 100);
                refreshTags();
            }).finally(() => {
                updating.current -= 1;
            }),
            {
                loading: 'Creating bookmark...',
                success: 'Created!',
                error: (err) => err.message,
            }
        );
    };

    const refreshTaskQueue = async (): Promise<boolean> => {
        const tasks = await api.fetchTaskQueue();
        const taskQueue = store.getState().taskQueue.value;
        dispatch(taskQueueSlice.update(tasks));
        return taskQueue.now! != tasks.now;
    };

    return {
        handleDelete,
        handleFetchMeta,
        handleSave,
        onCreate,
        refreshTaskQueue,
        updating,
    };
}