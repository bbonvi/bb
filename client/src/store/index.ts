// src/store/index.ts
import { configureStore } from '@reduxjs/toolkit';
import taskQueueReducer from './taskQueueSlice';
import bmarksReducer from './bmarksSlice';
import globalReducer from './globalSlice';

const store = configureStore({
    reducer: {
        taskQueue: taskQueueReducer,
        bmarks: bmarksReducer,
        global: globalReducer,
    },
});

export default store;

export type RootState = ReturnType<typeof store.getState>;
export type AppDispatch = typeof store.dispatch;

