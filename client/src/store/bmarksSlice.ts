import { createSlice, PayloadAction } from '@reduxjs/toolkit';
import { Bmark } from '../api';

interface BmarkState {
    value: Bmark[];
}

const initialState: BmarkState = {
    value: [],
};

const BmarkSlice = createSlice({
    name: 'Bmark',
    initialState,
    reducers: {
        updateAll: (state, action: PayloadAction<Bmark[]>) => {
            state.value = action.payload;
        },
        update: (state, action: PayloadAction<Bmark>) => {
            const idx = state.value.findIndex(b => b.id === action.payload.id);
            if (idx >= 0) {
                state.value[idx] = action.payload;
            }
        },
        create: (state, action: PayloadAction<Bmark>) => {
            state.value.push(action.payload);
        },
        remove: (state, action: PayloadAction<{ id: number }>) => {
            const idx = state.value.findIndex(b => b.id === action.payload.id);
            if (idx >= 0) {
                state.value.splice(idx, 1);
            }
        },
    },
});

export const { updateAll, update, remove, create } = BmarkSlice.actions;
export default BmarkSlice.reducer;

