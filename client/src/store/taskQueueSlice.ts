import { createSlice, PayloadAction } from '@reduxjs/toolkit';
import { TaskQueue } from '../api';

interface TaskQueueState {
    value: TaskQueue;
}

const initialState: TaskQueueState = {
    value: { now: 0, queue: [] },
};

const taskQueueSlice = createSlice({
    name: 'taskQueue',
    initialState,
    reducers: {
        update: (state, action: PayloadAction<TaskQueue>) => {
            state.value = action.payload;
        },
    },
});

export const { update } = taskQueueSlice.actions;
export default taskQueueSlice.reducer;

