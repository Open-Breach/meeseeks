/* A plain PipeWire consumer, to compare with pipewiresrc: does the producer's
 * pool starve after a renegotiation when the consumer is a bare pw_stream?
 *
 * It takes every buffer and hands it straight back.
 *
 * Usage: plain-consumer <target node id>
 * Environment:
 *   CONSUMER_RT=1     connect with PW_STREAM_FLAG_RT_PROCESS, as
 *                     gnome-remote-desktop does; without it the node is
 *                     asynchronous, as pipewiresrc's is
 *   CONSUMER_HOLD=N   keep the last N buffers before handing them back
 *                     (default 0)
 */
#include <stdio.h>
#include <stdlib.h>
#include <time.h>

#include <pipewire/pipewire.h>
#include <spa/param/video/format-utils.h>

#define MAX_HOLD 8

struct data {
	struct pw_main_loop *loop;
	struct pw_stream *stream;
	uint32_t frames, frames_total;
	struct timespec window;
	struct pw_buffer *held[MAX_HOLD];
	int n_held, hold;
};

static double since(const struct timespec *from)
{
	struct timespec now;
	clock_gettime(CLOCK_MONOTONIC, &now);
	return (now.tv_sec - from->tv_sec) + (now.tv_nsec - from->tv_nsec) / 1e9;
}

static void on_process(void *userdata)
{
	struct data *d = userdata;
	struct pw_buffer *b;

	while ((b = pw_stream_dequeue_buffer(d->stream)) != NULL) {
		d->frames++;
		d->frames_total++;
		if (d->hold > 0) {
			if (d->n_held == d->hold) {
				pw_stream_queue_buffer(d->stream, d->held[0]);
				for (int i = 1; i < d->n_held; i++)
					d->held[i - 1] = d->held[i];
				d->n_held--;
			}
			d->held[d->n_held++] = b;
		} else {
			pw_stream_queue_buffer(d->stream, b);
		}
	}
	if (d->window.tv_sec == 0)
		clock_gettime(CLOCK_MONOTONIC, &d->window);
	if (since(&d->window) >= 5.0) {
		fprintf(stderr, "consumer, last 5 s: %u frames\n", d->frames);
		d->frames = 0;
		clock_gettime(CLOCK_MONOTONIC, &d->window);
	}
}

static void on_state_changed(void *userdata, enum pw_stream_state old,
		enum pw_stream_state state, const char *error)
{
	struct data *d = userdata;
	fprintf(stderr, "consumer state: %s%s%s\n", pw_stream_state_as_string(state),
			error ? " error: " : "", error ? error : "");
	/* Buffers are gone when the stream stops; forget the ones we kept. */
	if (state != PW_STREAM_STATE_STREAMING)
		d->n_held = 0;
	if (state == PW_STREAM_STATE_ERROR || state == PW_STREAM_STATE_UNCONNECTED)
		pw_main_loop_quit(d->loop);
}

static void on_param_changed(void *userdata, uint32_t id, const struct spa_pod *param)
{
	struct data *d = userdata;
	struct spa_video_info_raw info;
	uint8_t buffer[1024];
	struct spa_pod_builder b = SPA_POD_BUILDER_INIT(buffer, sizeof(buffer));
	const struct spa_pod *params[1];

	if (param == NULL || id != SPA_PARAM_Format)
		return;
	if (spa_format_video_raw_parse(param, &info) < 0)
		return;
	fprintf(stderr, "consumer format: %ux%u\n", info.size.width, info.size.height);

	/* What pipewiresrc asks for: 16 by preference, anything from 1 up. */
	params[0] = spa_pod_builder_add_object(&b,
		SPA_TYPE_OBJECT_ParamBuffers, SPA_PARAM_Buffers,
		SPA_PARAM_BUFFERS_buffers, SPA_POD_CHOICE_RANGE_Int(16, 1, INT32_MAX),
		SPA_PARAM_BUFFERS_blocks, SPA_POD_CHOICE_RANGE_Int(0, 1, INT32_MAX),
		SPA_PARAM_BUFFERS_size, SPA_POD_CHOICE_RANGE_Int(0, 1, INT32_MAX),
		SPA_PARAM_BUFFERS_stride, SPA_POD_CHOICE_RANGE_Int(0, 0, INT32_MAX));
	pw_stream_update_params(d->stream, params, 1);
}

static const struct pw_stream_events stream_events = {
	PW_VERSION_STREAM_EVENTS,
	.state_changed = on_state_changed,
	.param_changed = on_param_changed,
	.process = on_process,
};

int main(int argc, char *argv[])
{
	struct data d = { 0 };
	uint8_t buffer[1024];
	struct spa_pod_builder b = SPA_POD_BUILDER_INIT(buffer, sizeof(buffer));
	const struct spa_pod *params[1];
	enum pw_stream_flags flags = PW_STREAM_FLAG_AUTOCONNECT | PW_STREAM_FLAG_MAP_BUFFERS;
	uint32_t target;

	if (argc < 2) {
		fprintf(stderr, "usage: plain-consumer <node id>\n");
		return 1;
	}
	target = (uint32_t)atoi(argv[1]);
	if (getenv("CONSUMER_RT"))
		flags |= PW_STREAM_FLAG_RT_PROCESS;
	if (getenv("CONSUMER_HOLD"))
		d.hold = SPA_CLAMP(atoi(getenv("CONSUMER_HOLD")), 0, MAX_HOLD);

	pw_init(&argc, &argv);
	d.loop = pw_main_loop_new(NULL);
	d.stream = pw_stream_new_simple(pw_main_loop_get_loop(d.loop), "plain-consumer",
			pw_properties_new(
				PW_KEY_MEDIA_TYPE, "Video",
				PW_KEY_MEDIA_CATEGORY, "Capture",
				PW_KEY_MEDIA_ROLE, "Screen",
				NULL),
			&stream_events, &d);

	params[0] = spa_pod_builder_add_object(&b,
		SPA_TYPE_OBJECT_Format, SPA_PARAM_EnumFormat,
		SPA_FORMAT_mediaType, SPA_POD_Id(SPA_MEDIA_TYPE_video),
		SPA_FORMAT_mediaSubtype, SPA_POD_Id(SPA_MEDIA_SUBTYPE_raw),
		SPA_FORMAT_VIDEO_format, SPA_POD_Id(SPA_VIDEO_FORMAT_BGRx),
		SPA_FORMAT_VIDEO_size, SPA_POD_Rectangle(&SPA_RECTANGLE(1920, 1080)),
		SPA_FORMAT_VIDEO_framerate, SPA_POD_Fraction(&SPA_FRACTION(0, 1)),
		SPA_FORMAT_VIDEO_maxFramerate, SPA_POD_CHOICE_RANGE_Fraction(
			&SPA_FRACTION(60, 1),
			&SPA_FRACTION(1, 1),
			&SPA_FRACTION(1000, 1)));

	fprintf(stderr, "consumer: %s%s, holding %d\n",
			(flags & PW_STREAM_FLAG_RT_PROCESS) ? "RT_PROCESS" : "asynchronous",
			"", d.hold);
	pw_stream_connect(d.stream, PW_DIRECTION_INPUT, target, flags, params, 1);
	pw_main_loop_run(d.loop);

	pw_stream_destroy(d.stream);
	pw_main_loop_destroy(d.loop);
	pw_deinit();
	return 0;
}
