#include <stdio.h>
#include <stdlib.h>
#include <X11/X.h>
#include <X11/Xlib.h>
#include <X11/keysym.h>
#include <GL/gl.h>
#include <GL/glx.h>
#include <unistd.h>

float xRot = 0.0f;
float yRot = 0.0f;
float zRot = 0.0f;
float scale = 1.0f;
int currentObject = 0; // 0=Triangle, 1=Rectangle, 2=Cube, 3=Octahedron

void drawTriangle() {
    glBegin(GL_TRIANGLES);
    // Basic R, G, B
    glColor3f(1.0f, 0.0f, 0.0f); glVertex3f( 0.0f,  1.0f, 0.0f);
    glColor3f(0.0f, 1.0f, 0.0f); glVertex3f(-1.0f, -1.0f, 0.0f);
    glColor3f(0.0f, 0.0f, 1.0f); glVertex3f( 1.0f, -1.0f, 0.0f);
    glEnd();
}

void drawRectangle() {
    glBegin(GL_QUADS);
    // More colors
    glColor3f(1.0f, 0.0f, 0.0f); glVertex3f(-1.0f,  1.0f, 0.0f);
    glColor3f(0.0f, 1.0f, 0.0f); glVertex3f( 1.0f,  1.0f, 0.0f);
    glColor3f(0.0f, 0.0f, 1.0f); glVertex3f( 1.0f, -1.0f, 0.0f);
    glColor3f(1.0f, 1.0f, 0.0f); glVertex3f(-1.0f, -1.0f, 0.0f);
    glEnd();
}

void drawCube() {
    float cv[8][3] = {
        {-1.0f, -1.0f,  1.0f}, { 1.0f, -1.0f,  1.0f}, 
        { 1.0f,  1.0f,  1.0f}, {-1.0f,  1.0f,  1.0f},
        {-1.0f, -1.0f, -1.0f}, { 1.0f, -1.0f, -1.0f}, 
        { 1.0f,  1.0f, -1.0f}, {-1.0f,  1.0f, -1.0f}
    };
    float cc[8][3] = {
        {1.0f, 0.0f, 0.0f}, {0.0f, 1.0f, 0.0f}, 
        {0.0f, 0.0f, 1.0f}, {1.0f, 1.0f, 0.0f},
        {1.0f, 0.0f, 1.0f}, {0.0f, 1.0f, 1.0f}, 
        {1.0f, 0.5f, 0.0f}, {0.5f, 0.0f, 1.0f} // Purple, Orange, etc.
    };
    int faces[6][4] = {
        {0,1,2,3}, {1,5,6,2}, {5,4,7,6}, 
        {4,0,3,7}, {3,2,6,7}, {4,5,1,0}
    };

    int i, j, v;

    glBegin(GL_QUADS);
    for(i = 0; i < 6; ++i) {
        for(j = 0; j < 4; ++j) {
            v = faces[i][j];
            glColor3fv(cc[v]);
            glVertex3fv(cv[v]);
        }
    }
    glEnd();
}

void drawOctahedron() {
    float ov[6][3] = {
        { 1.0f, 0.0f, 0.0f}, {-1.0f, 0.0f, 0.0f}, 
        { 0.0f, 1.0f, 0.0f}, { 0.0f,-1.0f, 0.0f}, 
        { 0.0f, 0.0f, 1.0f}, { 0.0f, 0.0f,-1.0f}
    };
    float oc[6][3] = {
        {1.0f, 0.2f, 0.5f}, {0.2f, 1.0f, 0.5f}, 
        {0.5f, 0.2f, 1.0f}, {1.0f, 1.0f, 0.0f}, 
        {0.0f, 1.0f, 1.0f}, {1.0f, 0.5f, 0.0f}
    };
    int ofaces[8][3] = {
        {0,2,4}, {0,4,3}, {0,3,5}, {0,5,2},
        {1,4,2}, {1,3,4}, {1,5,3}, {1,2,5}
    };

    int i, j, v;

    glBegin(GL_TRIANGLES);
    for(i = 0; i < 8; ++i) {
        for(j = 0; j < 3; ++j) {
            v = ofaces[i][j];
            glColor3fv(oc[v]);
            glVertex3fv(ov[v]);
        }
    }
    glEnd();
}

int main(int argc, char *argv[]) {
    Display                 *dpy;
    Window                  root;
    GLint                   att[] = { GLX_RGBA, GLX_DEPTH_SIZE, 24, GLX_ALPHA_SIZE, 0, None };
    GLint                   att_fb1[] = { GLX_RGBA, GLX_DEPTH_SIZE, 16, GLX_ALPHA_SIZE, 0, None };
    GLint                   att_fb2[] = { GLX_RGBA, GLX_DEPTH_SIZE, 12, GLX_ALPHA_SIZE, 0, None };
    GLint                   att_fb3[] = { GLX_RGBA, GLX_ALPHA_SIZE, 0, None };
    XVisualInfo             *vi;
    Colormap                cmap;
    XSetWindowAttributes    swa;
    Window                  win;
    GLXContext              glc;
    XEvent                  xev;

    dpy = XOpenDisplay(NULL);
    if(dpy == NULL) {
        printf("Cannot connect to X server\n");
        exit(1);
    }

    root = DefaultRootWindow(dpy);
    vi = glXChooseVisual(dpy, 0, att);
    if(vi == NULL) {
        printf("24-bit depth buffer visual not found, trying 16-bit...\n");
        vi = glXChooseVisual(dpy, 0, att_fb1);
    }
    if(vi == NULL) {
        printf("16-bit depth buffer visual not found, trying 12-bit...\n");
        vi = glXChooseVisual(dpy, 0, att_fb2);
    }
    if(vi == NULL) {
        printf("12-bit depth buffer visual not found, trying without explicit depth buffer...\n");
        vi = glXChooseVisual(dpy, 0, att_fb3);
    }
    if(vi == NULL) {
        printf("No appropriate RGBA visual found\n");
        exit(1);
    }

    cmap = XCreateColormap(dpy, root, vi->visual, AllocNone);
    swa.colormap = cmap;
    swa.event_mask = ExposureMask | KeyPressMask | StructureNotifyMask;
    swa.border_pixel = 0;
    
    win = XCreateWindow(dpy, root, 0, 0, 800, 600, 0, vi->depth, InputOutput, vi->visual, CWColormap | CWEventMask | CWBorderPixel, &swa);
    XMapWindow(dpy, win);
    XStoreName(dpy, win, "OpenGL 1.0 X11 Test");

    glc = glXCreateContext(dpy, vi, NULL, GL_TRUE);
    glXMakeCurrent(dpy, win, glc);

    // Init OpenGL state
    glEnable(GL_DEPTH_TEST);
    glShadeModel(GL_SMOOTH);
    glClearColor(0.4f, 0.1f, 0.6f, 1.0f); // Purple background

    while(1) {
        while(XPending(dpy)) {
            XNextEvent(dpy, &xev);
            
            if(xev.type == ConfigureNotify) {
                GLfloat ratio;
                // Setup Projection Matrix on resize
                glViewport(0, 0, xev.xconfigure.width, xev.xconfigure.height);
                glMatrixMode(GL_PROJECTION);
                glLoadIdentity();
                ratio = (GLfloat)xev.xconfigure.width / (GLfloat)xev.xconfigure.height;
                glFrustum(-ratio, ratio, -1.0, 1.0, 1.0, 100.0);
                glMatrixMode(GL_MODELVIEW);
            }
            else if(xev.type == KeyPress) {
                KeySym keysym = XLookupKeysym(&xev.xkey, 0);
                switch(keysym) {
                    // Rotation
                    case XK_Up:         xRot -= 5.0f; break;
                    case XK_Down:       xRot += 5.0f; break;
                    case XK_Left:       yRot -= 5.0f; break;
                    case XK_Right:      yRot += 5.0f; break;
                    case XK_Page_Up:    zRot -= 5.0f; break;
                    case XK_Page_Down:  zRot += 5.0f; break;
                    
                    // Scale
                    case XK_plus:
                    case XK_equal:
                    case XK_KP_Add:     
                        scale += 0.1f; 
                        break;
                    case XK_minus:
                    case XK_KP_Subtract: 
                        scale -= 0.1f; 
                        if (scale < 0.1f) scale = 0.1f;
                        break;

                    // Switch object
                    case XK_Insert:     
                        currentObject = (currentObject + 1) % 4; 
                        break;
                    case XK_Delete:     
                        currentObject = (currentObject - 1 + 4) % 4; 
                        break;

                    // Exit
                    case XK_Escape:
                        glXMakeCurrent(dpy, None, NULL);
                        glXDestroyContext(dpy, glc);
                        XDestroyWindow(dpy, win);
                        XCloseDisplay(dpy);
                        exit(0);
                        break;
                }
            }
        }

        // Render Frame
        glClear(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT);
        
        glLoadIdentity();
        glTranslatef(0.0f, 0.0f, -5.0f); // Move object into the view
        
        glRotatef(xRot, 1.0f, 0.0f, 0.0f);
        glRotatef(yRot, 0.0f, 1.0f, 0.0f);
        glRotatef(zRot, 0.0f, 0.0f, 1.0f);
        glScalef(scale, scale, scale);

        switch(currentObject) {
            case 0: drawTriangle(); break;
            case 1: drawRectangle(); break;
            case 2: drawCube(); break;
            case 3: drawOctahedron(); break;
        }

        glFlush();
        
        // Sleep for a short time to prevent maxing out the CPU (~60 FPS cap)
        usleep(16000); 
    }

    return 0;
}