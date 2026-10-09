package nl.neerdael.presetpoc;

import org.junit.Test;
import static org.junit.Assert.*;

public class TvNavigationTest {
    @Test public void openingPanelConsumesCenterReleaseBeforeNewFocusCanClick() {
        TvNavigation navigation = new TvNavigation();
        assertEquals(TvNavigation.Action.OPEN_PANEL, navigation.keyDown(23, false, false));
        assertEquals(TvNavigation.Action.CONSUME, navigation.keyDown(23, true, true));
        assertTrue(navigation.keyUp(23));
        assertFalse(navigation.keyUp(23));
        assertEquals(TvNavigation.Action.DELEGATE, navigation.keyDown(23, true, false));
        assertFalse(navigation.keyUp(23));
    }
    @Test public void heldBackClosesOnlyOnceThenNextBackCanExit() {
        TvNavigation navigation = new TvNavigation();
        assertEquals(TvNavigation.Action.CLOSE_PANEL, navigation.keyDown(4, true, false));
        assertEquals(TvNavigation.Action.CONSUME, navigation.keyDown(4, false, true));
        assertTrue(navigation.keyUp(4));
        assertEquals(TvNavigation.Action.DELEGATE, navigation.keyDown(4, false, false));
    }
    @Test public void closedPanelRoutesTransportAndCenter() {
        assertEquals(TvNavigation.Action.OPEN_PANEL, TvNavigation.route(23, false, false));
        assertEquals(TvNavigation.Action.NEXT, TvNavigation.route(22, false, false));
        assertEquals(TvNavigation.Action.PREVIOUS, TvNavigation.route(21, false, false));
        assertEquals(TvNavigation.Action.NEXT, TvNavigation.route(87, false, false));
        assertEquals(TvNavigation.Action.PREVIOUS, TvNavigation.route(88, false, false));
    }
    @Test public void panelKeepsCenterAndDirectionsWithFocusedControl() {
        for (int key : new int[]{23, 66, 19, 20, 21, 22})
            assertEquals(TvNavigation.Action.DELEGATE, TvNavigation.route(key, true, false));
    }
    @Test public void backClosesPanelBeforeActivityExit() {
        assertEquals(TvNavigation.Action.CLOSE_PANEL, TvNavigation.route(4, true, false));
        assertEquals(TvNavigation.Action.DELEGATE, TvNavigation.route(4, false, false));
        assertEquals(TvNavigation.Action.CLOSE_PANEL, TvNavigation.route(82, true, false));
    }
    @Test public void repeatingCenterCannotReopenPanel() {
        assertEquals(TvNavigation.Action.DELEGATE, TvNavigation.route(23, false, true));
        assertEquals(TvNavigation.Action.DELEGATE, TvNavigation.route(82, false, true));
    }
}
