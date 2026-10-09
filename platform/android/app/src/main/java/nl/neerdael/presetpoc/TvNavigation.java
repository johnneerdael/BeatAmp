package nl.neerdael.presetpoc;

/** Routes remote keys before focused controls; center stays with the focused row. */
final class TvNavigation {
    enum Action { CONSUME, DELEGATE, OPEN_PANEL, CLOSE_PANEL, PREVIOUS, NEXT, SHOW_INFO }

    private int consumedKey = -1;

    Action keyDown(int key, boolean panelOpen, boolean repeated) {
        if (repeated && key == consumedKey) return Action.CONSUME;
        Action action = route(key, panelOpen, repeated);
        if (action != Action.DELEGATE) consumedKey = key;
        return action;
    }

    boolean keyUp(int key) {
        if (key != consumedKey) return false;
        consumedKey = -1;
        return true;
    }

    static Action route(int key, boolean panelOpen, boolean repeated) {
        // Android keycode constants are stable; no View dependency keeps routing testable.
        if (panelOpen) {
            if (key == 4 || key == 82) return repeated ? Action.DELEGATE : Action.CLOSE_PANEL;
            return Action.DELEGATE;
        }
        if (key == 23 || key == 66 || key == 82) return repeated ? Action.DELEGATE : Action.OPEN_PANEL;
        if (key == 22 || key == 87 || key == 90) return Action.NEXT;
        if (key == 21 || key == 88 || key == 89) return Action.PREVIOUS;
        if (key == 19 || key == 20 || key == 165) return Action.SHOW_INFO;
        return Action.DELEGATE;
    }
}
