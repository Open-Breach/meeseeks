/* A video source that renegotiates its format shortly after it starts
 * streaming, to show what pipewiresrc does when that happens.
 *
 * It is modelled on how a compositor offers a screen cast stream (Mutter's
 * src/backends/meta-stream-source.c in particular):
 *  - the stream is a driver (PW_STREAM_FLAG_DRIVER), node.supports-request=2
 *  - the pool is small: SPA_POD_CHOICE_RANGE_Int(8, 2, 8) buffers
 *  - frames come from the main loop on a 60 Hz timer: dequeue a buffer, stamp
 *    it with CLOCK_MONOTONIC in SPA_META_Header, queue it
 *  - a tick that finds no free buffer is skipped and counted
 *  - with RENEGOTIATE_MS set, that many milliseconds after the stream first
 *    starts streaming the EnumFormat param is sent again with
 *    pw_stream_update_params(), which makes the link negotiate anew
 *
 * Every 5 seconds it prints how many frames it queued and how many ticks
 * found no free buffer.
 *
 * Build: cc -o renegotiating-source renegotiating-source.c \
 *            $(pkg-config --cflags --libs libpipewire-0.3)
 * Run:   RENEGOTIATE_MS=400 ./renegotiating-source
 *        then, with the node id it prints:
 *        gst-launch-1.0 pipewiresrc path=<id> ! video/x-raw,width=1920,height=1080 ! fakesink
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#include <pipewire/pipewire.h>
#include <spa/param/video/format-utils.h>

struct data {
	struct pw_main_loop *loop;
	struct pw_stream *stream;
	struct spa_source *tick;
	struct spa_source *renegotiate;
	struct spa_video_info_raw format;
	uint32_t stride;
	uint32_t seq;
	int renegotiated;
	uint32_t queued, starved;
	uint32_t queued_total, starved_total;
	struct timespec window;
	struct timespec started;
};

static double since(const struct timespec *from)
{
	struct timespec now;
	clock_gettime(CLOCK_MONOTONIC, &now);
	return (now.tv_sec - from->tv_sec) + (now.tv_nsec - from->tv_nsec) / 1e9;
}

static const struct spa_pod *build_format(struct spa_pod_builder *b)
{
	return spa_pod_builder_add_object(b,
		SPA_TYPE_OBJECT_Format, SPA_PARAM_EnumFormat,
		SPA_FORMAT_mediaType, SPA_POD_Id(SPA_MEDIA_TYPE_video),
		SPA_FORMAT_mediaSubtype, SPA_POD_Id(SPA_MEDIA_SUBTYPE_raw),
		SPA_FORMAT_VIDEO_format, SPA_POD_Id(SPA_VIDEO_FORMAT_BGRx),
		SPA_FORMAT_VIDEO_size, SPA_POD_CHOICE_RANGE_Rectangle(
			&SPA_RECTANGLE(1280, 720),
			&SPA_RECTANGLE(1, 1),
			&SPA_RECTANGLE(8192, 8192)),
		SPA_FORMAT_VIDEO_framerate, SPA_POD_Fraction(&SPA_FRACTION(0, 1)),
		SPA_FORMAT_VIDEO_maxFramerate, SPA_POD_CHOICE_RANGE_Fraction(
			&SPA_FRACTION(60, 1),
			&SPA_FRACTION(1, 1),
			&SPA_FRACTION(1000, 1)));
}

static void report(struct data *d, int final)
{
	if (!final && since(&d->window) < 5.0)
		return;
	printf("%s%u frames queued, %u ticks had no free buffer\n",
			final ? "total: " : "last 5 s: ",
			final ? d->queued_total : d->queued,
			final ? d->starved_total : d->starved);
	fflush(stdout);
	d->queued = d->starved = 0;
	clock_gettime(CLOCK_MONOTONIC, &d->window);
}

/* One 60 Hz tick: produce a frame if there is a buffer to put it in. */
static void on_tick(void *userdata, uint64_t expirations)
{
	struct data *d = userdata;
	struct pw_buffer *b;
	struct spa_buffer *buf;
	struct spa_meta_header *h;

	report(d, 0);

	if ((b = pw_stream_dequeue_buffer(d->stream)) == NULL) {
		d->starved++;
		d->starved_total++;
		return;
	}
	buf = b->buffer;
	if (buf->datas[0].data != NULL) {
		/* Touch one row so the frame is not all the same. */
		memset(buf->datas[0].data, (int)(d->seq & 0xff), d->stride);
	}
	if ((h = spa_buffer_find_meta_data(buf, SPA_META_Header, sizeof(*h)))) {
		struct timespec ts;
		clock_gettime(CLOCK_MONOTONIC, &ts);
		h->pts = SPA_TIMESPEC_TO_NSEC(&ts);
		h->flags = 0;
		h->seq = d->seq;
		h->dts_offset = 0;
	}
	d->seq++;
	buf->datas[0].chunk->offset = 0;
	buf->datas[0].chunk->size = d->format.size.height * d->stride;
	buf->datas[0].chunk->stride = d->stride;

	pw_stream_queue_buffer(d->stream, b);
	d->queued++;
	d->queued_total++;
}

static void on_renegotiate(void *userdata, uint64_t expirations)
{
	struct data *d = userdata;
	uint8_t buffer[1024];
	struct spa_pod_builder b = SPA_POD_BUILDER_INIT(buffer, sizeof(buffer));
	const struct spa_pod *params[1];

	params[0] = build_format(&b);
	printf("%.3f s: renegotiating (pw_stream_update_params); so far %u queued, %u ticks with no free buffer\n",
			since(&d->started), d->queued_total, d->starved_total);
	fflush(stdout);
	d->queued_total = d->starved_total = 0;
	pw_stream_update_params(d->stream, params, 1);
}

static void on_state_changed(void *userdata, enum pw_stream_state old,
		enum pw_stream_state state, const char *error)
{
	struct data *d = userdata;
	struct pw_loop *loop = pw_main_loop_get_loop(d->loop);
	struct timespec timeout = { 0, 0 }, interval = { 0, 0 };

	printf("state: %s%s%s (driving: %s)\n", pw_stream_state_as_string(state),
			error ? " error: " : "", error ? error : "",
			state == PW_STREAM_STATE_STREAMING ?
				(pw_stream_is_driving(d->stream) ? "yes" : "no") : "-");
	fflush(stdout);

	switch (state) {
	case PW_STREAM_STATE_PAUSED:
		if (old == PW_STREAM_STATE_CONNECTING)
			printf("node id: %u\n", pw_stream_get_node_id(d->stream));
		fflush(stdout);
		pw_loop_update_timer(loop, d->tick, NULL, NULL, false);
		break;
	case PW_STREAM_STATE_STREAMING:
		timeout.tv_nsec = 1;
		interval.tv_nsec = 16666667;
		pw_loop_update_timer(loop, d->tick, &timeout, &interval, false);
		if (d->started.tv_sec == 0) {
			clock_gettime(CLOCK_MONOTONIC, &d->started);
			clock_gettime(CLOCK_MONOTONIC, &d->window);
		}
		if (d->renegotiate && !d->renegotiated) {
			struct timespec after = { 0, 0 };
			long ms = atol(getenv("RENEGOTIATE_MS"));
			after.tv_sec = ms / 1000;
			after.tv_nsec = (ms % 1000) * SPA_NSEC_PER_MSEC;
			d->renegotiated = 1;
			pw_loop_update_timer(loop, d->renegotiate, &after, NULL, false);
		}
		break;
	case PW_STREAM_STATE_ERROR:
	case PW_STREAM_STATE_UNCONNECTED:
		pw_main_loop_quit(d->loop);
		break;
	default:
		break;
	}
}

static void on_param_changed(void *userdata, uint32_t id, const struct spa_pod *param)
{
	struct data *d = userdata;
	uint8_t buffer[1024];
	struct spa_pod_builder b = SPA_POD_BUILDER_INIT(buffer, sizeof(buffer));
	const struct spa_pod *params[2];

	if (param == NULL || id != SPA_PARAM_Format)
		return;
	if (spa_format_video_raw_parse(param, &d->format) < 0)
		return;

	d->stride = SPA_ROUND_UP_N(d->format.size.width * 4, 4);
	printf("format: %ux%u, max framerate %u/%u\n",
			d->format.size.width, d->format.size.height,
			d->format.max_framerate.num, d->format.max_framerate.denom);
	fflush(stdout);

	params[0] = spa_pod_builder_add_object(&b,
		SPA_TYPE_OBJECT_ParamBuffers, SPA_PARAM_Buffers,
		SPA_PARAM_BUFFERS_buffers, SPA_POD_CHOICE_RANGE_Int(8, 2, 8),
		SPA_PARAM_BUFFERS_blocks, SPA_POD_Int(1),
		SPA_PARAM_BUFFERS_size, SPA_POD_Int(d->stride * d->format.size.height),
		SPA_PARAM_BUFFERS_stride, SPA_POD_Int(d->stride));
	params[1] = spa_pod_builder_add_object(&b,
		SPA_TYPE_OBJECT_ParamMeta, SPA_PARAM_Meta,
		SPA_PARAM_META_type, SPA_POD_Id(SPA_META_Header),
		SPA_PARAM_META_size, SPA_POD_Int(sizeof(struct spa_meta_header)));
	pw_stream_update_params(d->stream, params, 2);
}

static void on_add_buffer(void *userdata, struct pw_buffer *buffer)
{
	static int n;
	printf("add buffer %d\n", ++n);
	fflush(stdout);
}

static const struct pw_stream_events stream_events = {
	PW_VERSION_STREAM_EVENTS,
	.state_changed = on_state_changed,
	.param_changed = on_param_changed,
	.add_buffer = on_add_buffer,
};

int main(int argc, char *argv[])
{
	struct data d = { 0 };
	uint8_t buffer[1024];
	struct spa_pod_builder b = SPA_POD_BUILDER_INIT(buffer, sizeof(buffer));
	const struct spa_pod *params[1];
	struct pw_loop *loop;

	pw_init(&argc, &argv);
	d.loop = pw_main_loop_new(NULL);
	loop = pw_main_loop_get_loop(d.loop);
	d.stream = pw_stream_new_simple(loop, "renegotiating-source",
			pw_properties_new(
				PW_KEY_MEDIA_CLASS, "Video/Source",
				"node.supports-request", "2",
				NULL),
			&stream_events, &d);
	d.tick = pw_loop_add_timer(loop, on_tick, &d);
	if (getenv("RENEGOTIATE_MS"))
		d.renegotiate = pw_loop_add_timer(loop, on_renegotiate, &d);

	params[0] = build_format(&b);
	pw_stream_connect(d.stream, PW_DIRECTION_OUTPUT, PW_ID_ANY,
			PW_STREAM_FLAG_DRIVER | PW_STREAM_FLAG_MAP_BUFFERS,
			params, 1);

	pw_main_loop_run(d.loop);
	report(&d, 1);

	pw_stream_destroy(d.stream);
	pw_main_loop_destroy(d.loop);
	pw_deinit();
	return 0;
}
