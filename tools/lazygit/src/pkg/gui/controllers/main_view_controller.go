package controllers

import (
	"regexp"
	"strings"

	"github.com/jesseduffield/lazygit/pkg/gocui"
	"github.com/jesseduffield/lazygit/pkg/gui/context"
	"github.com/jesseduffield/lazygit/pkg/gui/controllers/helpers"
	"github.com/jesseduffield/lazygit/pkg/gui/types"
)

type MainViewController struct {
	baseController
	c *ControllerCommon

	context      *context.MainContext
	otherContext *context.MainContext

	// With gui.mouseTextSelection, pressing the left button in the focused
	// main view starts a text selection, and the click's usual action runs on
	// release, only if no text was selected by then. This holds the press
	// until the release.
	pendingClick      *gocui.ViewMouseBindingOpts
	dragAutoscroller  *helpers.DragAutoscroller
	draggingWithMouse bool
	// column of the pointer at the last drag event, where the selection is
	// extended to when autoscrolling moves the line under the pointer
	lastDragX int
}

var _ types.IController = &MainViewController{}

func NewMainViewController(
	c *ControllerCommon,
	context *context.MainContext,
	otherContext *context.MainContext,
) *MainViewController {
	self := &MainViewController{
		baseController: baseController{},
		c:              c,
		context:        context,
		otherContext:   otherContext,
	}
	self.dragAutoscroller = helpers.NewDragAutoscroller(
		c.HelperCommon,
		context,
		self.canDragAutoscroll,
		self.handleDragAutoscroll,
	)
	return self
}

func (self *MainViewController) GetKeybindings(opts types.KeybindingsOpts) []*types.Binding {
	return []*types.Binding{
		{
			Keys:            opts.GetKeys(opts.Config.Universal.TogglePanel),
			Handler:         self.togglePanel,
			Description:     self.c.Tr.ToggleStagingView,
			Tooltip:         self.c.Tr.ToggleStagingViewTooltip,
			DisplayOnScreen: true,
		},
		{
			Keys:            opts.GetKeys(opts.Config.Universal.Return),
			Handler:         self.escape,
			Description:     self.c.Tr.ExitFocusedMainView,
			DisplayOnScreen: true,
		},
		{
			// overriding this because we want to read all of the task's output before we start searching
			Keys:        opts.GetKeys(opts.Config.Universal.StartSearch),
			Handler:     self.openSearch,
			Description: self.c.Tr.StartSearch,
			Tag:         "navigation",
		},
		{
			Keys:        opts.GetKeys(opts.Config.Universal.CopyToClipboard),
			Handler:     self.copySelection,
			Description: self.c.Tr.CopySelectedTextToClipboard,
		},
	}
}

func (self *MainViewController) GetMouseKeybindings(opts types.KeybindingsOpts) []*gocui.ViewMouseBinding {
	return []*gocui.ViewMouseBinding{
		{
			ViewName:    self.context.GetViewName(),
			Key:         gocui.MouseLeft,
			Handler:     self.onClickInAlreadyFocusedView,
			FocusedView: self.context.GetViewName(),
		},
		{
			ViewName:    self.context.GetViewName(),
			Key:         gocui.MouseLeft,
			Handler:     self.onClickInOtherViewOfMainViewPair,
			FocusedView: self.otherContext.GetViewName(),
		},
		{
			ViewName: self.context.GetViewName(),
			Key:      gocui.MouseLeft,
			Modifier: gocui.ModMotion,
			Handler:  self.onDrag,
		},
		{
			ViewName: self.context.GetViewName(),
			Key:      gocui.MouseRelease,
			Handler:  self.onRelease,
		},
	}
}

func (self *MainViewController) Context() types.Context {
	return self.context
}

func (self *MainViewController) GetOnFocusLost() func(types.OnFocusLostOpts) {
	return func(types.OnFocusLostOpts) {
		self.dragAutoscroller.Cancel()
		self.pendingClick = nil
		if self.draggingWithMouse {
			self.draggingWithMouse = false
			self.c.GocuiGui().CancelMouseCapture()
		}
	}
}

func (self *MainViewController) textSelectionEnabled() bool {
	return self.c.UserConfig().Gui.MouseTextSelection
}

func (self *MainViewController) togglePanel() error {
	if self.otherContext.GetView().Visible {
		self.c.Context().Push(self.otherContext, types.OnFocusOpts{})
	}

	return nil
}

func (self *MainViewController) escape() error {
	self.context.GetView().ClearTextSelection()
	self.c.Context().Pop()
	return nil
}

func (self *MainViewController) onClickInAlreadyFocusedView(opts gocui.ViewMouseBindingOpts) error {
	if !self.textSelectionEnabled() {
		return self.clickAction(opts)
	}

	// the press may be the start of a drag selection; act on release
	press := opts
	self.pendingClick = &press
	return nil
}

// clickAction is what a click in the focused main view does for the side
// panel that filled it, e.g. entering the staging view at the clicked line.
func (self *MainViewController) clickAction(opts gocui.ViewMouseBindingOpts) error {
	sidePanelContext := self.c.Context().NextInStack(self.context)
	if sidePanelContext != nil && sidePanelContext.GetOnClickFocusedMainView() != nil {
		return sidePanelContext.GetOnClickFocusedMainView()(self.context.GetViewName(), opts.Y)
	}
	return nil
}

func (self *MainViewController) onClickInOtherViewOfMainViewPair(opts gocui.ViewMouseBindingOpts) error {
	self.c.Context().Push(self.context, types.OnFocusOpts{
		ClickedWindowName:  self.context.GetWindowName(),
		ClickedViewLineIdx: opts.Y,
	})

	return nil
}

func (self *MainViewController) onDrag(opts gocui.ViewMouseBindingOpts) error {
	if !self.textSelectionEnabled() {
		return nil
	}

	self.draggingWithMouse = true
	self.lastDragX = opts.X
	originY := self.context.GetView().OriginY()
	self.dragAutoscroller.Update(opts.Y - originY)
	return nil
}

func (self *MainViewController) onRelease(opts gocui.ViewMouseBindingOpts) error {
	self.dragAutoscroller.Cancel()
	self.draggingWithMouse = false
	pending := self.pendingClick
	self.pendingClick = nil

	if !self.textSelectionEnabled() {
		return nil
	}

	view := self.context.GetView()
	if view.HasTextSelection() {
		return self.copySelection()
	}

	// a plain click: nothing was selected, so the click acts as usual
	view.ClearTextSelection()
	if pending != nil {
		return self.clickAction(*pending)
	}
	return nil
}

func (self *MainViewController) canDragAutoscroll(direction int) bool {
	view := self.context.GetView()
	if direction < 0 {
		return view.OriginY() > 0
	}
	return view.OriginY()+view.InnerHeight() < view.ViewLinesHeight()
}

// handleDragAutoscroll runs after the view scrolled by a line while dragging
// beyond its edge; viewIndex is the line now under the pointer.
func (self *MainViewController) handleDragAutoscroll(viewIndex int) bool {
	view := self.context.GetView()
	view.ExtendTextSelection(self.lastDragX, viewIndex)
	self.c.ReadLinesToFillView(view)
	return true
}

func (self *MainViewController) copySelection() error {
	view := self.context.GetView()
	text := view.SelectedText()
	if text == "" {
		return nil
	}

	self.c.LogAction(self.c.Tr.Actions.CopySelectedTextToClipboard)
	if err := self.c.OS().CopyToClipboard(dropDiffPrefix(stripDiffGutter(text))); err != nil {
		return err
	}
	self.c.Toast(self.c.Tr.SelectionCopiedToClipboard)
	return nil
}

// the line-number gutter of a pretty diff line (see the prettydiff package),
// up to and including its separator
var diffGutterRegexp = regexp.MustCompile(`^ *\d* +\d* *│`)

// stripDiffGutter removes the line numbers that pretty diffs show in front
// of each line, so that copied text is the diff's text.
func stripDiffGutter(text string) string {
	lines := strings.Split(text, "\n")
	for i, line := range lines {
		if loc := diffGutterRegexp.FindStringIndex(line); loc != nil {
			lines[i] = line[loc[1]:]
		}
	}
	return strings.Join(lines, "\n")
}

func (self *MainViewController) openSearch() error {
	if manager := self.c.GetViewBufferManagerForView(self.context.GetView()); manager != nil {
		manager.ReadToEnd(func() {
			self.c.OnUIThread(func() error {
				self.c.Helpers().Search.OpenSearchPrompt(self.context)
				return nil
			})
		})
	}

	return nil
}
