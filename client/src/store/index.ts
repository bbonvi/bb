// src/store/index.ts
import { configureStore } from '@reduxjs/toolkit';
import taskQueueReducer from './taskQueueSlice';

const store = configureStore({
    reducer: {
        taskQueue: taskQueueReducer,
    },
});

export default store;

export type RootState = ReturnType<typeof store.getState>;
export type AppDispatch = typeof store.dispatch;

